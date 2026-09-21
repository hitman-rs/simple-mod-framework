use std::{str::FromStr, sync::Arc};

use color_eyre::eyre::{OptionExt, Result, WrapErr, bail};
use ecow::EcoString;
use glacier_commons::{
	game::GlacierGame,
	metadata::{ReferenceFlags, ReferenceType, ResourceMetadata, ResourceReference, ResourceType, RuntimeID},
	resource_type, rid,
	rpkg_tool::RpkgResourceMeta
};
use indexmap::IndexMap;
use relative_path::RelativePath;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str, to_string};
use simple_mod_framework_core::{
	game::{GameContext, NominalPartition, ResourceSpecifier},
	resource_in,
	utils::ResultExt
};
use simple_mod_framework_types::{Localisation, ModID, NonEmptyString, VersionPlatform};
use tonytools::{clng::CLNG, ditl::DITL, dlge::DLGE, locr::LOCR, rtlv::RTLV};
use tryvial::try_fn;
use velcro::vec;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
	analysis::{ANALYSERS, Analyser},
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{Attribution, GraphOperation, Operation, RUNE_OPERATIONS, RuneOperation, rune_operation},
	register_operation,
	scripts::JsonValue,
	state::{Mutation, ResourceState, State},
	world::{Diagnostics, Mods, World}
};

fn check_and_insert(
	state: &Arc<State<impl World>>,
	attribution: &Attribution,
	obj: &mut serde_json::Map<String, Value>,
	lang: &str,
	key: &str,
	key_as_hash: &str,
	value: &str
) -> Result<()> {
	let obj = obj
		.get_mut(lang)
		.map(|x| x.as_object_mut().ok_or_eyre("HMLanguages language was not object"))
		.transpose()?;

	if let Some(obj) = obj {
		if obj.contains_key(key_as_hash) {
			// Overriding existing localisation with string key
			if obj
				.get(key_as_hash)
				.unwrap()
				.as_str()
				.expect("Localisation value was not string")
				== value && lang != "xx"
			{
				state.world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::LocalisationOverrideSameValue {
						lang: lang.to_string(),
						key: key.to_owned()
					},
					target: attribution.to_diagnostic_target()
				})?;
			}

			obj.insert(key_as_hash.to_owned(), Value::String(value.to_owned()));
		} else {
			// Overriding existing localisation with hash key OR creating new localisation with string/hash key
			if obj
				.get(key)
				.is_some_and(|x| x.as_str().expect("Localisation value was not string") == value)
				&& lang != "xx"
			{
				state.world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::LocalisationOverrideSameValue {
						lang: lang.to_string(),
						key: key.to_owned()
					},
					target: attribution.to_diagnostic_target()
				})?;
			}

			obj.insert((*key).to_owned(), Value::String(value.to_owned()));
		}
	}

	Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverwriteLanguageFile {
	#[rune(get, set)]
	pub id: ResourceSpecifier,

	#[rune(get, set)]
	pub filetype: ResourceType,

	#[rune(get, set)]
	pub data: JsonValue
}

register_operation!(OverwriteLanguageFile);

impl GraphOperation for OverwriteLanguageFile {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.id.to_owned()]
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(
			&serde_brief::to_vec_with_config(
				self,
				serde_brief::Config {
					use_indices: true,
					..Default::default()
				}
			)
			.expect("Couldn't serialise operation")
		)
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		log::trace!(target: attribution.source.as_str(), "Converting file");

		let tonytools::Rebuilt { file, meta } = match self.filetype.as_ref() {
			"CLNG" => {
				let clng = CLNG::new(state.game.version.into(), None).wrap_err("Couldn't create CLNG engine")?;

				clng.rebuild(to_string(&self.data)?)
					.wrap_err("Couldn't rebuild CLNG data")?
			}

			"DITL" => {
				let mut ditl =
					DITL::new((*state.localisation_hash_list).to_owned()).wrap_err("Couldn't create DITL engine")?;

				ditl.rebuild(to_string(&self.data)?)
					.wrap_err("Couldn't rebuild DITL data")?
			}

			"DLGE" => {
				let mut dlge = DLGE::new(
					(*state.localisation_hash_list).to_owned(),
					state.game.version.into(),
					None,
					None,
					false
				)
				.wrap_err("Couldn't create DLGE engine")?;

				dlge.rebuild(to_string(&self.data)?)
					.wrap_err("Couldn't rebuild DLGE data")?
			}

			"LOCR" => {
				let locr = LOCR::new(
					(*state.localisation_hash_list).to_owned(),
					state.game.version.into(),
					None,
					false
				)
				.wrap_err("Couldn't create LOCR engine")?;

				locr.rebuild(to_string(&self.data)?)
					.wrap_err("Couldn't rebuild LOCR data")?
			}

			"RTLV" => {
				let mut rtlv = RTLV::new(state.game.version.into(), None).wrap_err("Couldn't create RTLV engine")?;

				rtlv.rebuild(to_string(&self.data)?)
					.wrap_err("Couldn't rebuild RTLV data")?
			}

			_ => bail!("Unsupported language file type")
		};

		vec![Mutation::SetResourceValue {
			resource: self.id,
			value: ResourceState {
				metadata: ResourceMetadata::try_from(from_str::<RpkgResourceMeta>(&meta)?)?,
				data: file
			}
		}]
	}
}

#[try_fn]
fn analyse_overwrite_language(
	_: &dyn Mods,
	game: &GameContext,
	_: &ModID,
	attribution: &str,
	partition: &EcoString,
	file: &RelativePath,
	file_contents: Vec<u8>
) -> Result<Vec<Operation>> {
	let data: Value = serde_json::from_slice(&file_contents)
		.wrap_err("HMLanguages JSON was not valid JSON")
		.intentional()?;

	if let Some(spec) = game.realise_resource_specifier(
		RuntimeID::from_str(
			data.get("hash")
				.ok_or_eyre("HMLanguages file had no hash")
				.intentional()?
				.as_str()
				.ok_or_eyre("HMLanguages file hash key was not string")
				.intentional()?
		)?,
		NominalPartition(partition.to_owned())
	)? {
		vec![
			OverwriteLanguageFile {
				id: spec,
				filetype: file
					.file_name()
					.unwrap()
					.split('.')
					.nth(1)
					.unwrap()
					.to_uppercase()
					.try_into()?,
				data: data.into()
			}
			.into(),
		]
	} else {
		log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
		vec![]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_CLNG: Analyser = Analyser {
	file_type: "clng.json",
	analyse: analyse_overwrite_language
};

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_DITL: Analyser = Analyser {
	file_type: "ditl.json",
	analyse: analyse_overwrite_language
};

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_DLGE: Analyser = Analyser {
	file_type: "dlge.json",
	analyse: analyse_overwrite_language
};

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_LOCR: Analyser = Analyser {
	file_type: "locr.json",
	analyse: analyse_overwrite_language
};

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_RTLV: Analyser = Analyser {
	file_type: "rtlv.json",
	analyse: analyse_overwrite_language
};

const HUD_LOCALISATION_WOA: &str =
	"[assembly:/localization/hitman6/conversations/ui/pro/hud.sweetmenutext].pc_localized-textlist";

const HUD_LOCALISATION_FL: &str =
	"[assembly:/_knt/localization/knt/conversations/ui/ui_menutext.sweetmenutext].localized-textlist";

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct AddLocalisation {
	#[rune(get, set)]
	pub data: Vec<(NonEmptyString, Localisation)>
}

register_operation!(AddLocalisation);

impl GraphOperation for AddLocalisation {
	fn get_required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(HUD_LOCALISATION_FL, "super", platform)
		} else {
			resource_in!(HUD_LOCALISATION_WOA, "super", platform)
		}]
	}

	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(HUD_LOCALISATION_FL, "super", platform)
		} else {
			resource_in!(HUD_LOCALISATION_WOA, "super", platform)
		}]
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(
			&serde_brief::to_vec_with_config(
				self,
				serde_brief::Config {
					use_indices: true,
					..Default::default()
				}
			)
			.expect("Couldn't serialise operation")
		)
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		log::trace!(target: attribution.source.as_str(), "Acquiring resources");

		let res = &state
			.get_resource(&if state.game.version == GlacierGame::FL {
				resource_in!(HUD_LOCALISATION_FL, "super", &*state.game)
			} else {
				resource_in!(HUD_LOCALISATION_WOA, "super", &*state.game)
			})
			.await?;

		let locr = LOCR::new(
			(*state.localisation_hash_list).to_owned(),
			state.game.version.into(),
			None,
			false
		)
		.wrap_err("Couldn't create LOCR engine")?;

		let mut locr_data = locr
			.convert(
				&res.data,
				to_string(&RpkgResourceMeta::from_resource_metadata(
					res.metadata.to_owned().to_extended(&res.data, state.game.version)?,
					false
				))?
			)
			.wrap_err("Couldn't convert LOCR file")?;

		log::trace!(target: attribution.source.as_str(), "Patching localisation");

		let languages = &mut locr_data.languages;

		for (key, strings) in self.data {
			let key = key.to_uppercase();

			let first_specified = strings
				.first_specified()
				.ok_or_eyre("No languages specified")
				.intentional()?
				.to_owned();

			let english = strings.english;
			let french = strings.french;
			let italian = strings.italian;
			let german = strings.german;
			let spanish = strings.spanish;
			let spanish_mexico = strings.spanish_mexico;
			let portuguese_brazil = strings.portuguese_brazil;
			let turkish = strings.turkish;
			let polish = strings.polish;
			let russian = strings.russian;
			let chinese_simplified = strings.chinese_simplified;
			let chinese_traditional = strings.chinese_traditional;
			let japanese = strings.japanese;
			let korean = strings.korean;

			let english = if let Some(x) = english {
				x
			} else {
				first_specified.to_owned()
			};

			let french = if let Some(x) = french {
				x
			} else {
				first_specified.to_owned()
			};

			let italian = if let Some(x) = italian {
				x
			} else {
				first_specified.to_owned()
			};

			let german = if let Some(x) = german {
				x
			} else {
				first_specified.to_owned()
			};

			let spanish_mexico = if let Some(x) = spanish_mexico {
				x
			} else if let Some(x) = &spanish {
				// Fallback es-MX to es
				x.to_owned()
			} else {
				first_specified.to_owned()
			};

			let spanish = if let Some(x) = spanish {
				x
			} else {
				first_specified.to_owned()
			};

			let portuguese_brazil = if let Some(x) = portuguese_brazil {
				x
			} else {
				first_specified.to_owned()
			};

			let turkish = if let Some(x) = turkish {
				x
			} else {
				first_specified.to_owned()
			};

			let polish = if let Some(x) = polish {
				x
			} else {
				first_specified.to_owned()
			};

			let russian = if let Some(x) = russian {
				x
			} else {
				first_specified.to_owned()
			};

			let chinese_simplified = if let Some(x) = chinese_simplified {
				x
			} else {
				first_specified.to_owned()
			};

			let chinese_traditional = if let Some(x) = chinese_traditional {
				x
			} else {
				first_specified.to_owned()
			};

			let japanese = if let Some(x) = japanese {
				x
			} else {
				first_specified.to_owned()
			};

			let korean = if let Some(x) = korean {
				x
			} else {
				first_specified.to_owned()
			};

			languages
				.get_mut("xx")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(english.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("en")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(english.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("fr")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(french.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("it")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(italian.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("de")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(german.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("es")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(spanish.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("mx")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(spanish_mexico.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("br")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(portuguese_brazil.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("pl")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(polish.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("ru")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(russian.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("cn")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(chinese_simplified.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("jp")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(japanese.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("tc")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(chinese_traditional.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("ko")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(korean.as_str().into())
							)
					)
				})
				.transpose()?;

			languages
				.get_mut("tr")
				.map(|x| {
					color_eyre::eyre::Ok(
						x.as_object_mut()
							.ok_or_eyre("HMLanguages language was not object")?
							.insert(
								format!("{:X}", crc32fast::hash(key.as_bytes())),
								Value::String(turkish.as_str().into())
							)
					)
				})
				.transpose()?;
		}

		log::trace!(target: attribution.source.as_str(), "Rebuilding");

		let tonytools::Rebuilt { file, meta } = locr
			.rebuild(to_string(&locr_data)?)
			.wrap_err("Couldn't rebuild LOCR data")?;

		vec![Mutation::SetResourceValue {
			resource: if state.game.version == GlacierGame::FL {
				resource_in!(HUD_LOCALISATION_FL, "super", &*state.game)
			} else {
				resource_in!(HUD_LOCALISATION_WOA, "super", &*state.game)
			},
			value: ResourceState {
				metadata: ResourceMetadata::try_from(from_str::<RpkgResourceMeta>(&meta)?)?,
				data: file
			}
		}]
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverrideLocalisation {
	#[rune(get, set)]
	pub id: ResourceSpecifier,

	#[rune(get, set)]
	pub data: Vec<(NonEmptyString, Localisation)>
}

register_operation!(OverrideLocalisation);

impl GraphOperation for OverrideLocalisation {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.id.to_owned()]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.id.to_owned()]
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(
			&serde_brief::to_vec_with_config(
				self,
				serde_brief::Config {
					use_indices: true,
					..Default::default()
				}
			)
			.expect("Couldn't serialise operation")
		)
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		log::trace!(target: attribution.source.as_str(), "Acquiring resources");

		let res = &state.get_resource(&self.id).await?;

		let locr = LOCR::new(
			tonytools::hashlist::HashList {
				tags: Default::default(),
				switches: Default::default(),
				lines: Default::default(),
				version: u32::MAX
			},
			state.game.version.into(),
			None,
			false
		)
		.wrap_err("Couldn't create LOCR engine")?;

		let mut locr_data = locr
			.convert(
				&res.data,
				to_string(&RpkgResourceMeta::from_resource_metadata(
					res.metadata.to_owned().to_extended(&res.data, state.game.version)?,
					false
				))?
			)
			.wrap_err("Couldn't convert LOCR file")?;

		log::trace!(target: attribution.source.as_str(), "Patching localisation");

		let languages = &mut locr_data.languages;

		for (key, strings) in self.data {
			let key = key.to_uppercase();
			let key_as_hash = format!("{:X}", crc32fast::hash(key.as_bytes()));

			let mut check_and_insert = |lang: &str, value: &str| {
				check_and_insert(&state, &attribution, languages, lang, &key, &key_as_hash, value)
			};

			if let Some(value) = strings.english {
				check_and_insert("xx", &value)?;
				check_and_insert("en", &value)?;
			}

			if let Some(value) = strings.french {
				check_and_insert("fr", &value)?;
			}

			if let Some(value) = strings.italian {
				check_and_insert("it", &value)?;
			}

			if let Some(value) = strings.german {
				check_and_insert("de", &value)?;
			}

			if let Some(value) = strings.spanish {
				check_and_insert("es", &value)?;
			}

			if let Some(value) = strings.spanish_mexico {
				check_and_insert("mx", &value)?;
			}

			if let Some(value) = strings.portuguese_brazil {
				check_and_insert("br", &value)?;
			}

			if let Some(value) = strings.polish {
				check_and_insert("pl", &value)?;
			}

			if let Some(value) = strings.russian {
				check_and_insert("ru", &value)?;
			}

			if let Some(value) = strings.chinese_simplified {
				check_and_insert("cn", &value)?;
			}

			if let Some(value) = strings.japanese {
				check_and_insert("jp", &value)?;
			}

			if let Some(value) = strings.chinese_traditional {
				check_and_insert("tc", &value)?;
			}

			if let Some(value) = strings.korean {
				check_and_insert("ko", &value)?;
			}

			if let Some(value) = strings.turkish {
				check_and_insert("tr", &value)?;
			}
		}

		log::trace!(target: attribution.source.as_str(), "Rebuilding");

		let tonytools::Rebuilt { file, meta } = locr
			.rebuild(to_string(&locr_data)?)
			.wrap_err("Couldn't rebuild LOCR data")?;

		vec![Mutation::SetResourceValue {
			resource: self.id,
			value: ResourceState {
				metadata: ResourceMetadata::try_from(from_str::<RpkgResourceMeta>(&meta)?)?,
				data: file
			}
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_LOCALISATION_PATCH: Analyser = Analyser {
	file_type: "localisation.patch.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: Value = serde_json::from_slice(&file_contents)
			.wrap_err("Localisation patch JSON was not valid JSON")
			.intentional()?;

		if let Some(spec) = game.realise_resource_specifier(
			RuntimeID::from_str(
				data.get("id")
					.ok_or_eyre("Localisation patch had no id key")
					.intentional()?
					.as_str()
					.ok_or_eyre("Localisation patch id key was not string")
					.intentional()?
			)?,
			NominalPartition(partition.to_owned())
		)? {
			Ok(vec![
				OverrideLocalisation {
					id: spec,
					data: serde_json::from_value::<IndexMap<NonEmptyString, Localisation>>(
						data.get("lines")
							.ok_or_eyre("Localisation patch had no lines key")
							.intentional()?
							.to_owned()
					)
					.wrap_err("Localisation patch lines were not valid")?
					.into_iter()
					.collect()
				}
				.into(),
			])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct WriteLocalisedLine {
	#[rune(get, set)]
	pub line: ResourceSpecifier,

	#[rune(get, set)]
	pub loc: String
}

register_operation!(WriteLocalisedLine);

impl GraphOperation for WriteLocalisedLine {
	fn should_cache(&self) -> bool {
		false
	}

	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.line.to_owned()]
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(
			&serde_brief::to_vec_with_config(
				self,
				serde_brief::Config {
					use_indices: true,
					..Default::default()
				}
			)
			.expect("Couldn't serialise operation")
		)
	}

	#[try_fn]
	async fn evaluate(self, _: Attribution, _: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		vec![Mutation::SetResourceValue {
			resource: self.line.to_owned(),
			value: ResourceState {
				metadata: ResourceMetadata {
					id: self.line.id,
					resource_type: resource_type!("LINE"),
					compressed: ResourceMetadata::infer_compressed(resource_type!("LINE")),
					scrambled: ResourceMetadata::infer_scrambled(resource_type!("LINE")),
					references: vec![ResourceReference {
						resource: rid!(HUD_LOCALISATION_WOA),
						flags: ReferenceFlags {
							reference_type: ReferenceType::Install,
							acquired: false,
							language_code: 0x1F
						}
					}]
				},
				data: vec![..crc32fast::hash(self.loc.as_bytes()).to_le_bytes(), 0x0] // extra null byte
			}
		}]
	}
}
