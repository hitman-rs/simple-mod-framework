#![feature(try_find)]

mod meta;
mod qn;
mod v2;

use std::{fs, ops::Deref, path::Path};

use color_eyre::eyre::{self, OptionExt, Result, WrapErr, bail, eyre};
use ecow::{EcoString, eco_format};
use fn_wrap_err::wrap_err;
use glacier_commons::{game::GlacierGame, metadata::RuntimeID};
use glacier_formats::{
	material::{
		Binder, BlendMode, ClassFlags, InstanceFlags, MaterialEntity, MaterialInstance, MaterialOverride,
		MaterialPropertyValue, RenderState
	},
	wwev::WwiseEvent
};
use indexmap::IndexMap;
use lazy_regex::{regex_captures, regex_replace, regex_replace_all};
pub use meta::*;
pub use qn::*;
use relative_path::{PathExt, RelativePathBuf};
use rpkg_rs::resource::partition_manager::PartitionManager;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value, json, to_string, to_value};
use serde_json5::from_slice;
use simple_mod_framework_core::utils::format_json;
use simple_mod_framework_types::{
	HashMap, Manifest, ManifestConditions, ManifestData, ModID, ModOption, ModOptionData, ModReference, NonEmptyVec,
	PackageDefinitionEntity, Platform, SafeRelativePath, SelectionOption, SemVer, UIText, UrlOrNull, VersionPlatform,
	VersionRange
};
use specta::Type;
use tryvial::try_fn;
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

/// Sanitise an arbitrary string into a valid reasonably short filename without spaces.
fn file_sanitise(name: &str) -> String {
	name.chars()
		.flat_map(|c| {
			if c.is_ascii_alphanumeric() {
				Some(c)
			} else if c == ' ' {
				Some('-')
			} else {
				None
			}
		})
		.take(16)
		.collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModInfo {
	pub url: String,
	pub version: SemVer
}

#[try_fn]
pub fn upgrade_mod(
	game_files: &PartitionManager,
	tthl: &tonytools::hashlist::HashList,
	mods: &HashMap<String, ModInfo>,
	mod_folder: impl AsRef<Path>,
	latest_framework_version: Version,
	progress: impl Fn(&str) + Send + Sync
) -> Result<()> {
	let mod_folder = mod_folder.as_ref();

	if mod_folder.join("project.json").exists()
		&& let Ok(contents) = fs::read(mod_folder.join("project.json"))
		&& let Ok(mut parsed) = from_slice::<Value>(&contents)
		&& let Some(paths) = parsed
			.get_mut("customPaths")
			.and_then(|x| from_value::<Vec<String>>(x.take()).ok())
	{
		for path in paths {
			// Register all custom paths with the global hash list
			// Allows them to be reserialised as paths in entities/the manifest
			if path.starts_with('[') {
				RuntimeID::from_path(&path);
			}
		}
	}

	let manifest_path = mod_folder.join("manifest.json");

	let mut manifest = from_slice::<Value>(&fs::read(&manifest_path).wrap_err("Couldn't read manifest file")?)
		.wrap_err("Manifest is invalid JSON")?;

	if manifest
		.get("frameworkVersion")
		.and_then(|x| x.as_str())
		.ok_or_eyre("frameworkVersion is not string")?
		.starts_with('1')
	{
		progress("Upgrading manifest from v1 to v2");
		upgrade_v1_manifest_to_v2(&mut manifest)?;
		fs::write(mod_folder.join("manifest.json"), format_json(&to_string(&manifest)?)?)?;
	}

	if manifest
		.get("frameworkVersion")
		.and_then(|x| x.as_str())
		.ok_or_eyre("frameworkVersion is not string")?
		.starts_with('2')
	{
		progress("Upgrading mod from v2 to v3");
		upgrade_v2_mod(
			game_files,
			tthl,
			mods,
			mod_folder,
			from_value(manifest).wrap_err("v2 manifest is invalid")?,
			latest_framework_version,
			progress
		)?;
	}
}

#[try_fn]
#[wrap_err("Couldn't upgrade manifest from v1 to v2")]
pub fn upgrade_v1_manifest_to_v2(manifest: &mut Value) -> Result<()> {
	let manifest = manifest.as_object_mut().unwrap();

	if let Some(value) = manifest.remove("contentFolder") {
		manifest.insert("contentFolders".into(), vec![value].into());
	}

	if let Some(value) = manifest.remove("blobsFolder") {
		manifest.insert("blobsFolders".into(), vec![value].into());
	}

	if let Some(dependencies) = manifest.get_mut("dependencies").and_then(|x| x.as_array_mut()) {
		for dependency in dependencies {
			if let Some(data) = dependency.as_object_mut() {
				let to_chunk = data
					.get("toChunk")
					.and_then(|x| x.as_str())
					.ok_or_eyre("toChunk is not string")?
					.replace("chunk", "")
					.parse::<u8>()
					.wrap_err("toChunk was not in form chunkX")?;

				data.insert("toChunk".into(), to_chunk.into());
			}
		}
	}

	if let Some(options) = manifest.get_mut("options").and_then(|x| x.as_array_mut()) {
		for option in options {
			let option = option.as_object_mut().ok_or_eyre("Option is not an object")?;

			if let Some(value) = option.remove("contentFolder") {
				option.insert("contentFolders".into(), vec![value].into());
			}

			if let Some(value) = option.remove("blobsFolder") {
				option.insert("blobsFolders".into(), vec![value].into());
			}

			if let Some(dependencies) = option.get_mut("dependencies").and_then(|x| x.as_array_mut()) {
				for dependency in dependencies {
					if let Some(data) = dependency.as_object_mut() {
						let to_chunk = data
							.get("toChunk")
							.and_then(|x| x.as_str())
							.ok_or_eyre("toChunk is not string")?
							.replace("chunk", "")
							.parse::<u8>()
							.wrap_err("toChunk was not in form chunkX")?;

						data.insert("toChunk".into(), to_chunk.into());
					}
				}
			}

			if option
				.get("type")
				.and_then(|x| x.as_str())
				.ok_or_eyre("Option type is not string")?
				== "requirement"
			{
				option.insert("type".into(), "conditional".into());

				let mods = option
					.remove("mods")
					.and_then(|x| from_value::<Vec<String>>(x).ok())
					.ok_or_eyre("Requirement option mods list is not array of strings")?;

				option.insert(
					"condition".into(),
					mods.into_iter()
						.map(|x| format!(r#""{x}" in config.loadOrder"#))
						.collect::<Vec<_>>()
						.join(" and ")
						.into()
				);
			}
		}
	}

	manifest.insert("frameworkVersion".into(), "2.0.0".into());
}

#[wrap_err("Invalid mod id {id}")]
pub fn reformat_mod_id(id: &str) -> Result<ModID> {
	EcoString::from(regex_replace_all!(
		r"\.([a-z])",
		&regex_replace_all!(
			r"_|-",
			&regex_replace_all!(r"(?:_|-)([a-z])", id, |_, x: &str| x.to_uppercase()),
			""
		),
		|_, x: &str| format!(".{}", x.to_uppercase())
	))
	.try_into()
	.map_err(|e: &str| eyre!(e))
}

#[try_fn]
#[wrap_err("Couldn't upgrade mod from v2 to v3")]
pub fn upgrade_v2_mod(
	game_files: &PartitionManager,
	tthl: &tonytools::hashlist::HashList,
	mods: &HashMap<String, ModInfo>,
	mod_folder: impl AsRef<Path>,
	old_manifest: v2::manifest::Manifest,
	latest_framework_version: Version,
	progress: impl Fn(&str) + Send + Sync
) -> Result<()> {
	let mod_folder = mod_folder.as_ref();

	let mod_info = mods.get(old_manifest.id.deref());

	// Register all paths
	for entry in old_manifest.packagedefinition.as_deref().unwrap_or(&vec![]) {
		if let Some(entry) = &entry.subtype_1 {
			let path = regex_replace!(
				r"\.(?:pc_)?([a-zA-Z]+)$",
				entry.path.as_ref().ok_or_eyre("Packagedefinition path is missing")?,
				".pc_$1"
			);

			if path.starts_with('[') {
				RuntimeID::from_path(&path);
			}

			let path = regex_replace!(
				r"\.(?:pc_)?entity(?:type|template)$",
				entry.path.as_ref().ok_or_eyre("Packagedefinition path is missing")?,
				".pc_entityblueprint"
			);

			if path.starts_with('[') {
				RuntimeID::from_path(&path);
			}
		}
	}

	for option in &old_manifest.options {
		let packagedefinition = match option {
			v2::manifest::ManifestOptionsItem::Variant0 { packagedefinition, .. } => packagedefinition,
			v2::manifest::ManifestOptionsItem::Variant1 { packagedefinition, .. } => packagedefinition,
			v2::manifest::ManifestOptionsItem::Variant2 { packagedefinition, .. } => packagedefinition
		};

		for entry in packagedefinition.as_deref().unwrap_or(&vec![]) {
			if let Some(entry) = &entry.subtype_1 {
				let path = regex_replace!(
					r"\.(?:pc_)?([a-zA-Z]+)$",
					entry.path.as_ref().ok_or_eyre("Packagedefinition path is missing")?,
					".pc_$1"
				);

				if path.starts_with('[') {
					RuntimeID::from_path(&path);
				}

				let path = regex_replace!(
					r"\.(?:pc_)?entity(?:type|template)$",
					entry.path.as_ref().ok_or_eyre("Packagedefinition path is missing")?,
					".pc_entityblueprint"
				);

				if path.starts_with('[') {
					RuntimeID::from_path(&path);
				}
			}
		}
	}

	let (conditions, data, mut path_renames) = upgrade_v2_data(
		game_files,
		tthl,
		mods,
		mod_folder,
		"manifest",
		v2::manifest::ManifestOptionData {
			blobs_folders: old_manifest.blobs_folders,
			content_folders: old_manifest.content_folders,
			dependencies: old_manifest.dependencies,
			incompatibilities: old_manifest.incompatibilities,
			load_after: old_manifest.load_after,
			load_before: old_manifest.load_before,
			localisation: old_manifest.localisation,
			localisation_overrides: old_manifest.localisation_overrides,
			localised_lines: old_manifest.localised_lines,
			packagedefinition: old_manifest.packagedefinition,
			peacock_plugins: old_manifest.peacock_plugins,
			requirements: old_manifest.requirements,
			scripts: old_manifest.scripts,
			supported_platforms: old_manifest.supported_platforms,
			thumbs: old_manifest.thumbs
		},
		&progress
	)?;

	let mut new_manifest = Manifest {
		id: reformat_mod_id(&old_manifest.id)?,
		name: UIText::from_english(
			EcoString::from(old_manifest.name.trim())
				.try_into()
				.map_err(|e: &str| eyre!(e))?
		),
		description: UIText::from_english(
			EcoString::from(old_manifest.description.trim())
				.try_into()
				.map_err(|e: &str| eyre!(e))?
		),
		authors: NonEmptyVec(
			old_manifest
				.authors
				.into_iter()
				.map(|author| EcoString::from(author).try_into().map_err(|e: &str| eyre!(e)))
				.collect::<Result<Vec<_>>>()?
				.try_into()?
		),
		version: SemVer(old_manifest.version.parse()?),
		framework_version: SemVer(latest_framework_version),
		url: if let Some(update_check) = old_manifest.update_check
			&& let Some((_, author, repo)) = regex_captures!(r"https://github\.com/(.*?)/(.*?)/", &update_check)
		{
			UrlOrNull(Some(format!("https://github.com/{author}/{repo}").parse()?))
		} else {
			UrlOrNull(mod_info.as_ref().map(|x| x.url.parse()).transpose()?)
		},
		links: Default::default(),
		conditions: ManifestConditions {
			supported_games: if conditions.supported_games.is_empty() {
				[
					VersionPlatform {
						version: GlacierGame::H3,
						platform: Platform::Epic
					},
					VersionPlatform {
						version: GlacierGame::H3,
						platform: Platform::Steam
					},
					VersionPlatform {
						version: GlacierGame::H3,
						platform: Platform::Microsoft
					}
				]
				.into_iter()
				.collect()
			} else {
				conditions.supported_games
			},
			required_mods: conditions.required_mods,
			required_conditions: conditions.required_conditions,
			incompatible_mods: conditions.incompatible_mods,
			incompatible_conditions: conditions.incompatible_conditions
		},
		data,
		options: vec![],
		presets: vec![]
	};

	let option_data = old_manifest
		.options
		.iter()
		.cloned()
		.map(|option| match option {
			v2::manifest::ManifestOptionsItem::Variant0 {
				blobs_folders,
				content_folders,
				dependencies,
				incompatibilities,
				load_after,
				load_before,
				localisation,
				localisation_overrides,
				localised_lines,
				packagedefinition,
				peacock_plugins,
				requirements,
				scripts,
				supported_platforms,
				thumbs,
				name,
				..
			} => upgrade_v2_data(
				game_files,
				tthl,
				mods,
				mod_folder,
				&name,
				v2::manifest::ManifestOptionData {
					blobs_folders,
					content_folders,
					dependencies,
					incompatibilities,
					load_after,
					load_before,
					localisation,
					localisation_overrides,
					localised_lines,
					packagedefinition,
					peacock_plugins,
					requirements,
					scripts,
					supported_platforms,
					thumbs
				},
				&progress
			)
			.wrap_err_with(|| format!("Couldn't upgrade option data for {}", name.as_str())),

			v2::manifest::ManifestOptionsItem::Variant1 {
				blobs_folders,
				content_folders,
				dependencies,
				incompatibilities,
				load_after,
				load_before,
				localisation,
				localisation_overrides,
				localised_lines,
				packagedefinition,
				peacock_plugins,
				requirements,
				scripts,
				supported_platforms,
				thumbs,
				name,
				..
			} => upgrade_v2_data(
				game_files,
				tthl,
				mods,
				mod_folder,
				&name,
				v2::manifest::ManifestOptionData {
					blobs_folders,
					content_folders,
					dependencies,
					incompatibilities,
					load_after,
					load_before,
					localisation,
					localisation_overrides,
					localised_lines,
					packagedefinition,
					peacock_plugins,
					requirements,
					scripts,
					supported_platforms,
					thumbs
				},
				&progress
			)
			.wrap_err_with(|| format!("Couldn't upgrade option data for {}", name.as_str())),

			v2::manifest::ManifestOptionsItem::Variant2 {
				blobs_folders,
				content_folders,
				dependencies,
				incompatibilities,
				load_after,
				load_before,
				localisation,
				localisation_overrides,
				localised_lines,
				packagedefinition,
				peacock_plugins,
				requirements,
				scripts,
				supported_platforms,
				thumbs,
				name,
				..
			} => upgrade_v2_data(
				game_files,
				tthl,
				mods,
				mod_folder,
				&name,
				v2::manifest::ManifestOptionData {
					blobs_folders,
					content_folders,
					dependencies,
					incompatibilities,
					load_after,
					load_before,
					localisation,
					localisation_overrides,
					localised_lines,
					packagedefinition,
					peacock_plugins,
					requirements,
					scripts,
					supported_platforms,
					thumbs
				},
				&progress
			)
			.wrap_err_with(|| format!("Couldn't upgrade option data for {}", name.as_str()))
		})
		.collect::<Result<Vec<_>>>()?;

	for (option, (conditions, data, renames)) in old_manifest.options.into_iter().zip(option_data) {
		path_renames.extend(renames);
		match option {
			v2::manifest::ManifestOptionsItem::Variant0 {
				enabled_by_default,
				image,
				name,
				tooltip,
				..
			} => {
				// Checkbox
				let name = EcoString::from(name.trim());
				new_manifest.options.push(ModOption {
					id: eco_format!("change-this-to-something-else-{:x}", xxh3_64(name.as_bytes()))
						.try_into()
						.map_err(|e: &str| eyre!(e))?,
					data: ModOptionData::Boolean {
						name: UIText::from_english(name.to_owned().try_into().map_err(|e: &str| eyre!(e))?),
						description: tooltip
							.map(|x| {
								EcoString::from(x.trim())
									.try_into()
									.map(UIText::from_english)
									.map_err(|e: &str| eyre!(e))
							})
							.transpose()?,
						default_value: enabled_by_default.unwrap_or(false),
						image: image
							.map(|x| {
								RelativePathBuf::from_path(Path::new(&String::from(x)))?
									.try_into()
									.map_err(|e: &str| eyre!(e))
							})
							.transpose()?,
						conditions,
						data
					}
				});
			}

			v2::manifest::ManifestOptionsItem::Variant1 {
				enabled_by_default,
				group,
				image,
				name,
				tooltip,
				..
			} => {
				// Select
				let group = EcoString::from(group.trim());
				let name = EcoString::from(name.trim());

				let new_option = SelectionOption {
					id: eco_format!(
						"change-this-to-something-else-{:x}",
						xxh3_64(format!("{}:{}", group, name.as_str()).as_bytes())
					)
					.try_into()
					.map_err(|e: &str| eyre!(e))?,
					name: UIText::from_english(name.to_owned().try_into().map_err(|e: &str| eyre!(e))?),
					description: tooltip
						.map(|x| {
							EcoString::from(x.trim())
								.try_into()
								.map(UIText::from_english)
								.map_err(|e: &str| eyre!(e))
						})
						.transpose()?,
					image: image
						.map(|x| {
							RelativePathBuf::from_path(Path::new(&String::from(x)))?
								.try_into()
								.map_err(|e: &str| eyre!(e))
						})
						.transpose()?,
					conditions,
					data
				};

				let group_id = eco_format!("change-this-to-something-else-{:x}", xxh3_64(group.as_bytes()));

				if let Some(ModOption {
					data: ModOptionData::Selection {
						default_value, options, ..
					},
					..
				}) = new_manifest.options.iter_mut().find(
					|opt| matches!(opt, ModOption { id, data: ModOptionData::Selection { .. } } if **id == group_id)
				) {
					if enabled_by_default.unwrap_or(false) {
						*default_value = new_option.id.to_owned();
					}

					options.push(new_option);
				} else {
					new_manifest.options.push(ModOption {
						id: group_id.try_into().map_err(|e: &str| eyre!(e))?,
						data: ModOptionData::Selection {
							name: UIText::from_english(group.try_into().map_err(|e: &str| eyre!(e))?),
							description: None,
							default_value: new_option.id.to_owned(),
							conditions: Default::default(),
							data: Default::default(),
							options: NonEmptyVec(vec![new_option].try_into()?)
						}
					});
				}
			}

			v2::manifest::ManifestOptionsItem::Variant2 { condition, name, .. } => {
				// Conditional
				new_manifest.options.push(ModOption {
					id: eco_format!("change-this-to-something-else-{:x}", xxh3_64(name.as_bytes()))
						.try_into()
						.map_err(|e: &str| eyre!(e))?,
					data: ModOptionData::Conditional {
						condition: EcoString::from(
							String::from(condition)
								.split(" and ")
								.map(|x| {
									Ok({
										if let Some((_, mod_id)) =
											regex_captures!(r#""(.+?)" not in config\.loadOrder"#, x)
										{
											format!(
												r#"!mod_enabled(`{}@{}`)"#,
												reformat_mod_id(mod_id)?,
												mods.get(mod_id).ok_or_else(|| eyre!("Unknown mod {mod_id}"))?.version
											)
										} else if let Some((_, mod_id)) =
											regex_captures!(r#""(.+?)" in config\.loadOrder"#, x)
										{
											format!(
												r#"mod_enabled(`{}@{}`)"#,
												reformat_mod_id(mod_id)?,
												mods.get(mod_id).ok_or_else(|| eyre!("Unknown mod {mod_id}"))?.version
											)
										} else {
											return Err(eyre!(
												"Option condition cannot be automatically upgraded: {}",
												x
											));
										}
									})
								})
								.collect::<Result<Vec<_>>>()?
								.join(" && ")
						)
						.try_into()
						.map_err(|e: &str| eyre!(e))?,
						data
					}
				});
			}
		}
	}

	fn handle_rename(path_renames: &[(RelativePathBuf, RelativePathBuf)], path: &mut SafeRelativePath) {
		if let Some((old, new)) = path_renames.iter().find(|(old, _)| path.starts_with(old)) {
			*path = new.join(path.strip_prefix(old).unwrap()).try_into().unwrap();
		}
	}

	fn handle_renames_data(path_renames: &[(RelativePathBuf, RelativePathBuf)], data: &mut ManifestData) {
		data.peacock_plugins
			.iter_mut()
			.for_each(|x| handle_rename(path_renames, x));
	}

	fn handle_renames_option(path_renames: &[(RelativePathBuf, RelativePathBuf)], option: &mut ModOption) {
		match &mut option.data {
			ModOptionData::Boolean { data, image, .. } => {
				if let Some(x) = image.as_mut() {
					handle_rename(path_renames, x)
				}

				handle_renames_data(path_renames, data);
			}

			ModOptionData::Selection { options, .. } => {
				options.iter_mut().for_each(|option| {
					if let Some(x) = option.image.as_mut() {
						handle_rename(path_renames, x)
					}

					handle_renames_data(path_renames, &mut option.data);
				});
			}

			ModOptionData::Conditional { data, .. } => {
				handle_renames_data(path_renames, data);
			}

			ModOptionData::Number { image, .. } => {
				if let Some(x) = image.as_mut() {
					handle_rename(path_renames, x)
				}
			}

			ModOptionData::Color { image, .. } => {
				if let Some(x) = image.as_mut() {
					handle_rename(path_renames, x)
				}
			}

			ModOptionData::String { image, .. } => {
				if let Some(x) = image.as_mut() {
					handle_rename(path_renames, x)
				}
			}

			ModOptionData::OptionGroup { options, data, .. } => {
				options.iter_mut().for_each(|x| handle_renames_option(path_renames, x));
				handle_renames_data(path_renames, data);
			}
		}
	}

	handle_renames_data(&path_renames, &mut new_manifest.data);
	new_manifest
		.options
		.iter_mut()
		.for_each(|x| handle_renames_option(&path_renames, x));

	// Collect sections into option groups
	let mut collect_again = true;
	while collect_again {
		collect_again = false;

		fn split_option_name(name: &str) -> Option<(EcoString, EcoString)> {
			if let Some((_, section, name)) = regex_captures!(r"^(.+) ?\| ?(.+?)$", name)
				&& !section.trim().is_empty()
				&& !name.trim().is_empty()
			{
				Some((section.trim().into(), name.trim().into()))
			} else if let Some((_, section, name)) = regex_captures!(r"^(.+) - (.+?)$", name)
				&& !section.trim().is_empty()
				&& !name.trim().is_empty()
			{
				Some((section.trim().into(), name.trim().into()))
			} else {
				None
			}
		}

		let mut groups = vec![];

		new_manifest.options.retain_mut(|option| {
			if let Some(name) = match &mut option.data {
				ModOptionData::Boolean { name, .. } => Some(name),
				ModOptionData::Selection { name, .. } => Some(name),
				ModOptionData::Number { name, .. } => Some(name),
				ModOptionData::Color { name, .. } => Some(name),
				ModOptionData::String { name, .. } => Some(name),
				ModOptionData::OptionGroup { name, .. } => Some(name),
				ModOptionData::Conditional { .. } => None
			} && let Some((section, option_name)) = split_option_name(name.english().unwrap())
			{
				*name = UIText::from_english(option_name.try_into().unwrap());

				groups.push(ModOption {
					id: eco_format!(
						"change-this-to-something-else-{:x}",
						xxh3_64(format!("section-{}", section).as_bytes())
					)
					.try_into()
					.unwrap(),
					data: ModOptionData::OptionGroup {
						display_condition: None,
						name: UIText::from_english(section.try_into().unwrap()),
						description: None,
						hidden_description: None,
						options: NonEmptyVec(vec![option.to_owned()].try_into().unwrap()),
						presets: Default::default(),
						data: Default::default()
					}
				});

				collect_again = true;

				false
			} else {
				true
			}
		});

		new_manifest.options.extend(groups);
	}

	// Condense option groups
	fn condense_groups(options: Vec<ModOption>) -> Vec<ModOption> {
		let mut new_options = vec![];
		for option in options {
			if let ModOption {
				id,
				data:
					ModOptionData::OptionGroup {
						display_condition,
						name,
						description,
						hidden_description,
						options,
						presets,
						data
					}
			} = option
			{
				if let Some(ModOption {
					data: ModOptionData::OptionGroup {
						options: existing_options,
						..
					},
					..
				}) = new_options.iter_mut().find(
					|x| matches!(x, ModOption { data: ModOptionData::OptionGroup { name: n, .. }, .. } if *n == name)
				) {
					existing_options.extend(options.0);
					*existing_options = NonEmptyVec(
						condense_groups(existing_options.0.to_owned().into())
							.try_into()
							.unwrap()
					);
				} else {
					new_options.push(ModOption {
						id,
						data: ModOptionData::OptionGroup {
							display_condition,
							name,
							description,
							hidden_description,
							options: NonEmptyVec(condense_groups(options.0.into()).try_into().unwrap()),
							presets,
							data
						}
					});
				}
			} else {
				new_options.push(option);
			}
		}

		new_options
	}

	new_manifest.options = condense_groups(new_manifest.options);

	fs::write(
		mod_folder.join("manifest.json"),
		format_json(&to_string(&new_manifest)?)?
	)?;
}

#[try_fn]
pub fn upgrade_v2_data(
	game_files: &PartitionManager,
	tthl: &tonytools::hashlist::HashList,
	mods: &HashMap<String, ModInfo>,
	mod_folder: impl AsRef<Path>,
	name: &str,
	data: v2::manifest::ManifestOptionData,
	progress: impl Fn(&str) + Send + Sync
) -> Result<(
	ManifestConditions,
	ManifestData,
	Vec<(RelativePathBuf, RelativePathBuf)>
)> {
	let mod_folder = mod_folder.as_ref();

	if !data.thumbs.is_empty()
		&& data.thumbs != vec!["ConsoleCmd OnlineResources_Disable 1".to_owned()]
		&& data.thumbs != vec!["ConsoleCmd OnlineResources_Disable 0".to_owned()]
	{
		bail!("Thumbs command cannot be automatically upgraded");
	}

	if !data.scripts.is_empty() {
		bail!("Scripts cannot be automatically upgraded");
	}

	let process_mod_reference = |item: v2::manifest::ModReferenceArrayItem| {
		eyre::Ok(ModReference {
			id: reformat_mod_id(match &item {
				v2::manifest::ModReferenceArrayItem::Array(x, _) => x,
				v2::manifest::ModReferenceArrayItem::String(x) => x
			})?,
			version: match item {
				v2::manifest::ModReferenceArrayItem::Array(_, x) => VersionRange(x.parse()?),
				v2::manifest::ModReferenceArrayItem::String(id) => VersionRange(
					mods.get(&id)
						.ok_or_else(|| eyre!("Unknown mod {id}"))?
						.version
						.to_string()
						.parse()?
				)
			}
		})
	};

	let mut path_renames = vec![];

	let mut content_folders: Vec<SafeRelativePath> = data
		.content_folders
		.map(Vec::from)
		.unwrap_or_default()
		.into_iter()
		.map(|x| {
			RelativePathBuf::from_path(Path::new(&x.replace("\\", "/")))?
				.try_into()
				.map_err(|e: &str| eyre!(e))
		})
		.collect::<Result<_>>()?;

	for content_folder in &content_folders {
		let content_folder = content_folder.to_logical_path(mod_folder);

		let mut chunk_folders = fs::read_dir(&content_folder)?.collect::<Result<Vec<_>, _>>()?;
		chunk_folders.sort_by_key(|x| x.file_name());

		chunk_folders.iter().try_for_each(|chunk_folder| {
			let entries = WalkDir::new(chunk_folder.path())
				.sort_by_file_name()
				.into_iter()
				.collect::<Result<Vec<_>, _>>()?;

			entries
				.into_iter()
				.filter(|entry| entry.metadata().is_ok_and(|m| m.is_file()))
				.try_for_each(|file| {
					upgrade_v2_content_file(game_files, mod_folder, file.path(), &progress)
						.wrap_err_with(|| eyre!("Couldn't upgrade file {}", file.path().to_string_lossy()))
				})
		})?;

		for chunk_folder in chunk_folders {
			if let Some((_, chunk_idx)) = regex_captures!(r#"chunk(\d+)"#, &chunk_folder.file_name().to_string_lossy())
			{
				let chunk_idx = chunk_idx.parse::<usize>()?;

				let partition_name = game_files
					.partitions
					.get(chunk_idx)
					.ok_or_else(|| eyre!("Unknown chunk {chunk_idx}"))?
					.partition_info()
					.name
					.as_deref()
					.ok_or_eyre("Partition has no name")?;

				fs::rename(chunk_folder.path(), content_folder.join(partition_name))?;

				path_renames.push((
					chunk_folder.path().relative_to(mod_folder)?,
					content_folder.join(partition_name).relative_to(mod_folder)?
				));
			}
		}
	}

	for (file, localisation_override) in data.localisation_overrides {
		// Make a content folder if none exist
		if content_folders.is_empty() {
			content_folders.push(
				RelativePathBuf::from(format!(
					"content-renameMe-{}-{:x}",
					file_sanitise(name),
					xxh3_64(format!("{}{}", file, to_string(&localisation_override)?).as_bytes())
				))
				.try_into()
				.map_err(|e: &str| eyre!(e))?
			);
		}

		let mut localisation: IndexMap<String, IndexMap<String, String>> = IndexMap::new();

		for (language, strings) in
			from_value::<IndexMap<String, IndexMap<String, String>>>(to_value(localisation_override)?)?
		{
			for (key, value) in strings {
				let key: u32 = key.parse().wrap_err_with(|| eyre!("Invalid localisation key {key}"))?;

				let key = tthl
					.lines
					.get_by_left(&key)
					.or_else(|| tthl.switches.get_by_left(&key))
					.or_else(|| tthl.tags.get_by_left(&key))
					.cloned()
					.unwrap_or(format!("{:08X}", key));

				localisation.entry(key).or_default().insert(language.to_owned(), value);
			}
		}

		let file = file.parse::<RuntimeID>()?;

		let partition_folder =
			content_folders.first().unwrap().to_logical_path(mod_folder).join(
				get_resource_partition(game_files, file)?.ok_or_else(|| eyre!("No such localisation file {file}"))?
			);

		fs::create_dir_all(&partition_folder)?;
		fs::write(
			partition_folder.join(format!(
				"{}.localisation.patch.json",
				file.get_path()
					.and_then(|path| path
						.replace("].pc_localized-textlist", "")
						.replace(".sweetmenutext", "")
						.split('/')
						.next_back()
						.map(|x| format!("{}-{}", file_sanitise(x), file.to_hash())))
					.unwrap_or_else(|| file.to_hash())
			)),
			format_json(&to_string(&json!({
				"id": file,
				"lines": localisation
			}))?)?
		)?;
	}

	(
		ManifestConditions {
			supported_games: data
				.supported_platforms
				.into_iter()
				.map(|x| VersionPlatform {
					version: GlacierGame::H3,
					platform: match x {
						v2::manifest::Platform::Epic => Platform::Epic,
						v2::manifest::Platform::Steam => Platform::Steam,
						v2::manifest::Platform::Microsoft => Platform::Microsoft
					}
				})
				.collect(),
			required_mods: data
				.requirements
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(process_mod_reference)
				.collect::<Result<Vec<_>>>()?,
			incompatible_mods: data
				.incompatibilities
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(process_mod_reference)
				.collect::<Result<Vec<_>>>()?,
			..Default::default()
		},
		ManifestData {
			content_folders,
			blob_folders: data
				.blobs_folders
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(|x| {
					RelativePathBuf::from_path(Path::new(&x.replace("\\", "/")))?
						.try_into()
						.map_err(|e: &str| eyre!(e))
				})
				.collect::<Result<_>>()?,
			localisation: data
				.localisation
				.map(|original| {
					let mut localisation: IndexMap<String, IndexMap<String, String>> = IndexMap::new();

					for (language, strings) in
						from_value::<IndexMap<String, IndexMap<String, String>>>(to_value(original).unwrap()).unwrap()
					{
						for (key, value) in strings {
							localisation.entry(key).or_default().insert(language.to_owned(), value);
						}
					}

					from_value(to_value(localisation).unwrap()).unwrap()
				})
				.unwrap_or_default(),
			localised_lines: data
				.localised_lines
				.into_iter()
				.map(|(id, line)| {
					Ok((
						id.parse()?,
						EcoString::from(line).try_into().map_err(|e: &str| eyre!(e))?
					))
				})
				.collect::<Result<_>>()?,
			package_definition: data
				.packagedefinition
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(|entry| {
					if let Some(entry) = entry.subtype_1 {
						Ok(PackageDefinitionEntity {
							partition: EcoString::from(
								entry.partition.ok_or_eyre("Packagedefinition partition is missing")?
							)
							.try_into()
							.map_err(|e: &str| eyre!(e))?,
							path: EcoString::from(entry.path.ok_or_eyre("Packagedefinition path is missing")?)
								.try_into()
								.map_err(|e: &str| eyre!(e))?
						})
					} else {
						bail!("Packagedefinition partitions are no longer supported");
					}
				})
				.collect::<Result<Vec<_>>>()?,
			// Dependencies as used in v2 are not needed in v3
			port_resources: vec![],
			deploy_before: data
				.load_before
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(process_mod_reference)
				.collect::<Result<Vec<_>>>()?,
			deploy_after: data
				.load_after
				.map(Vec::from)
				.unwrap_or_default()
				.into_iter()
				.map(process_mod_reference)
				.collect::<Result<Vec<_>>>()?,
			peacock_plugins: data
				.peacock_plugins
				.into_iter()
				.map(|x| {
					RelativePathBuf::from_path(Path::new(&x.replace("\\", "/")))?
						.try_into()
						.map_err(|e: &str| eyre!(e))
				})
				.collect::<Result<_>>()?,
			sdk_mods: Default::default(),
			scripts: vec![]
		},
		path_renames
	)
}

#[try_fn]
pub fn upgrade_v2_content_file(
	game_files: &PartitionManager,
	mod_folder: impl AsRef<Path>,
	file_path: impl AsRef<Path>,
	progress: impl Fn(&str)
) -> Result<()> {
	let mod_folder = mod_folder.as_ref();
	let file_path = file_path.as_ref();

	progress(&format!("Processing {}", file_path.to_string_lossy()));

	let filename = file_path
		.file_name()
		.ok_or_eyre("File has no name")?
		.to_str()
		.ok_or_eyre("File name is invalid")?;

	match filename {
		filename if filename.ends_with(".entity.json") && !filename.ends_with(".material.entity.json") => {
			upgrade_entity(file_path)?;
		}

		filename if filename.ends_with(".entity.patch.json") => {
			upgrade_patch(game_files, mod_folder, file_path)?;
		}

		filename if filename.ends_with(".JSON.patch.json") => {
			let mut data = from_slice::<IndexMap<String, Value>>(&fs::read(file_path)?)?;

			if data.contains_key("file") {
				let id = to_value(
					data.shift_remove("file")
						.as_ref()
						.and_then(|x| x.as_str())
						.ok_or_eyre("file key not string")?
						.parse::<RuntimeID>()?
				)?;

				data.insert("id".into(), id);
				data.shift_remove("type");

				fs::write(file_path, format_json(&to_string(&data)?)?)?;
			}
		}

		filename if filename.ends_with(".tga.meta") => {
			upgrade_texture_meta(file_path)?;
		}

		filename if filename.ends_with(".material.json") => {
			let data = from_slice::<Value>(&fs::read(file_path)?)?;
			if data.get("MATI").is_some() {
				let data: v2::material::Material = from_value(data)?;

				let material = data.material.ok_or_eyre("Material is missing")?;

				let instance = material.instance.first().ok_or_eyre("Material instance is missing")?;

				fs::write(
					file_path,
					format_json(&to_string(&MaterialInstance {
						id: data.mati.parse()?,
						name: instance.name.to_owned(),
						material_type: data.type_.parse()?,
						tags: instance.tags.to_owned(),
						class: (!data.mate.is_empty()).then(|| data.mate.parse()).transpose()?,
						descriptor: (!data.eres.is_empty()).then(|| data.eres.parse()).transpose()?,
						class_flags: {
							let flags = data
								.flags
								.as_ref()
								.ok_or_eyre("Flags missing")?
								.class
								.as_ref()
								.ok_or_eyre("Class flags missing")?;

							ClassFlags {
								reflection_2d: flags.reflection2d.unwrap_or(false),
								refraction_2d: flags.refraction2d.unwrap_or(false),
								lighting: flags.lighting.unwrap_or(false),
								emissive: flags.emissive.unwrap_or(false),
								discard: flags.discard.unwrap_or(false),
								lm_skin: flags.lm_skin.unwrap_or(false),
								prim_standard: flags.primclass_standard.unwrap_or(false),
								prim_linked: flags.primclass_linked.unwrap_or(false),
								prim_weighted: flags.primclass_weighted.unwrap_or(false),
								dof_override: flags.dofoverride.unwrap_or(false),
								uses_default_vs: flags.uses_default_vs.unwrap_or(false),
								uses_sprite_sa_vs: flags.uses_sprite_sa_vs.unwrap_or(false),
								uses_sprite_ao_vs: flags.uses_sprite_ao_vs.unwrap_or(false),
								alpha: flags.alpha.unwrap_or(false),
								uses_simple_shader: flags.uses_simple_shader.unwrap_or(false),
								disable_instancing: flags.disable_instancing.unwrap_or(false),
								lm_hair: flags.lm_hair.unwrap_or(false),
								sample_lighting: flags.unknown_1.or(flags.sample_lighting).unwrap_or(false),
								horizon_mapping: flags.unknown_2.or(flags.horizonmapping).unwrap_or(false),
								unknown_1: flags.unknown_3.unwrap_or(false),
								unknown_2: flags.unknown_4.unwrap_or(false),
								unknown_3: flags.unknown_5.unwrap_or(false)
							}
						},
						instance_flags: {
							let flags = data
								.flags
								.as_ref()
								.ok_or_eyre("Flags missing")?
								.instance
								.as_ref()
								.ok_or_eyre("Instance flags missing")?;

							InstanceFlags {
								opaque_emissive: flags.opaque_emissive.unwrap_or(false),
								trans_emissive: flags.trans_emissive.unwrap_or(false),
								trans_add_emissive: flags.transadd_emissive.unwrap_or(false),
								opaque_lit: flags.opaque_lit.unwrap_or(false),
								trans_lit: flags.trans_lit.unwrap_or(false),
								decal: flags.decal.unwrap_or(false),
								refractive: flags.refractive.unwrap_or(false),
								lm_skin: flags.lm_skin.unwrap_or(false),
								lm_hair: flags.lm_hair.unwrap_or(false),
								force_emissive: flags.force_emissive.unwrap_or(false),
								disable_shader_lod: flags.disable_shader_lod.unwrap_or(false),
								discard: flags.discard.unwrap_or(false),
								decal_emissive: flags.decal_emissive.unwrap_or(false),
								water_clipping: flags.water_clipping.unwrap_or(false),
								sample_lighting: flags.sample_lighting.unwrap_or(false),
								exclude_global_shadows: flags.exclude_global_shadows.unwrap_or(false)
							}
						},
						binder: {
							let binder = instance.binder.first().ok_or_eyre("Binder is missing")?;

							Binder {
								render_state: {
									let render_state =
										binder.render_state.first().ok_or_eyre("Render state is missing")?;

									RenderState {
										name: render_state.name.to_owned(),
										enabled: render_state.enabled.map(|x| x == 1),
										blend_enabled: render_state.blend_enabled.map(|x| x == 1),
										blend_mode: render_state
											.blend_mode
											.as_ref()
											.map(|x| {
												Ok(match x.as_ref() {
													"ADD" => BlendMode::Add,
													"TRANS" => BlendMode::Trans,
													"TRANS_PREMULTIPLIED_ALPHA" => BlendMode::TransPremultipliedAlpha,
													"TRANS_ON_OPAQUE" => BlendMode::TransOnOpaque,
													"OPAQUE" => BlendMode::Opaque,
													_ => bail!("Invalid blend mode: {x}")
												})
											})
											.transpose()?,
										decal_blend_diffuse: render_state
											.decal_blend_diffuse
											.map(|x| x.try_into())
											.transpose()?,
										decal_blend_normal: render_state
											.decal_blend_normal
											.map(|x| x.try_into())
											.transpose()?,
										decal_blend_specular: render_state
											.decal_blend_specular
											.map(|x| x.try_into())
											.transpose()?,
										decal_blend_roughness: render_state
											.decal_blend_roughness
											.map(|x| x.try_into())
											.transpose()?,
										decal_blend_emission: render_state
											.decal_blend_emission
											.map(|x| x.try_into())
											.transpose()?,
										alpha_test_enabled: render_state.alpha_test_enabled.map(|x| x == 1),
										alpha_reference: render_state
											.alpha_reference
											.map(|x| x.try_into())
											.transpose()?,
										fog_enabled: render_state.fog_enabled.map(|x| x == 1),
										opacity: render_state.opacity.map(|x| x as f32),
										culling_mode: render_state
											.culling_mode
											.as_ref()
											.ok_or_eyre("Culling mode is missing")?
											.parse()?,
										z_bias: render_state.z_bias.map(|x| x.try_into()).transpose()?,
										z_offset: render_state.z_offset.map(|x| x as f32),
										subsurface_red: render_state.subsurface_red.map(|x| x as f32),
										subsurface_green: render_state.subsurface_green.map(|x| x as f32),
										subsurface_blue: render_state.subsurface_blue.map(|x| x as f32),
										subsurface_value: render_state.subsurface_value.map(|x| x as f32)
									}
								},
								properties: binder
									.float_value
									.iter()
									.map(|x| {
										Ok((
											x.name.to_owned().ok_or_eyre("Property name is missing")?,
											match x.value.as_ref().ok_or_eyre("Property value is missing")? {
												v2::material::FloatTypesValue::Number(value) => {
													MaterialPropertyValue::Float {
														enabled: x.enabled.ok_or_eyre("Property enabled is missing")?
															== 1,
														value: *value as f32
													}
												}

												v2::material::FloatTypesValue::Array(value) => {
													MaterialPropertyValue::Vector {
														enabled: x.enabled.ok_or_eyre("Property enabled is missing")?
															== 1,
														value: value
															.iter()
															.map(|x| {
																Ok(x.as_f64().ok_or_eyre("Invalid vector value")?
																	as f32)
															})
															.collect::<Result<_>>()?
													}
												}
											}
										))
									})
									.chain(binder.texture.iter().map(|x| {
										Ok((
											x.name.to_owned().ok_or_eyre("Property name is missing")?,
											MaterialPropertyValue::Texture {
												enabled: x.enabled.ok_or_eyre("Property enabled is missing")? == 1,
												value: x
													.texture_id
													.as_ref()
													.and_then(|x| (!x.is_empty()).then_some(x))
													.map(|x| x.parse())
													.transpose()?,
												tiling_u: x.tiling_u.to_owned().unwrap_or_default(),
												tiling_v: x.tiling_v.to_owned().unwrap_or_default(),
												texture_type: x.type_.to_owned().unwrap_or_default()
											}
										))
									}))
									.chain(binder.color.iter().map(|x| {
										let v2::material::ColorTypesValue::Array(value) =
											x.value.as_ref().ok_or_eyre("Color value is missing")?
										else {
											bail!("Invalid color value");
										};

										let value = value
											.iter()
											.map(|x| x.as_f64().ok_or_eyre("Invalid color value"))
											.collect::<Result<Vec<_>>>()?;

										Ok((
											x.name.to_owned().ok_or_eyre("Property name is missing")?,
											MaterialPropertyValue::Colour {
												enabled: x.enabled.ok_or_eyre("Property enabled is missing")? == 1,
												value: format!(
													"#{}",
													value
														.into_iter()
														.map(|x| format!("{:02x}", (x * 255.0).round() as u8))
														.collect::<Vec<_>>()
														.join("")
												)
											}
										))
									}))
									.collect::<Result<_>>()?
							}
						}
					})?)?
				)?;

				if let Some(matt) = data.matt
					&& !matt.is_empty()
					&& let Some(matb) = data.matb
					&& !matb.is_empty()
				{
					fs::write(
						file_path.with_file_name(filename.replace(".material.json", ".material.entity.json")),
						format_json(&to_string(&MaterialEntity {
							factory: matt.parse()?,
							blueprint: matb.parse()?,
							material: data.mati.parse()?,
							overrides: data
								.overrides
								.get("Texture")
								.and_then(|x| x.as_object())
								.into_iter()
								.flatten()
								.map(|(x, y)| {
									Ok((
										x.to_owned(),
										MaterialOverride::Texture(
											y.as_str()
												.and_then(|x| (!x.is_empty()).then_some(x))
												.map(|x| x.parse())
												.transpose()?
										)
									))
								})
								.chain(
									data.overrides
										.get("Color")
										.and_then(|x| x.as_object())
										.into_iter()
										.flatten()
										.map(|(x, y)| {
											let value = y
												.as_array()
												.ok_or_eyre("Invalid color value")?
												.iter()
												.map(|x| x.as_f64().ok_or_eyre("Invalid color value"))
												.collect::<Result<Vec<_>>>()?;

											Ok((
												x.to_owned(),
												MaterialOverride::Color(
													value
														.into_iter()
														.map(|x| format!("{:02x}", (x * 255.0).round() as u8))
														.collect::<Vec<_>>()
														.join("")
												)
											))
										})
								)
								.chain(
									data.overrides
										.iter()
										.filter(|&(x, _)| x != "Texture" && x != "Color")
										.map(|(x, y)| {
											Ok((
												x.to_owned(),
												if y.is_number() {
													MaterialOverride::Float(
														y.as_f64().ok_or_eyre("Invalid float value")? as f32
													)
												} else {
													MaterialOverride::Vector(
														y.as_array()
															.ok_or_eyre("Invalid vector value")?
															.iter()
															.map(|x| {
																Ok(x.as_f64().ok_or_eyre("Invalid vector value")?
																	as f32)
															})
															.collect::<Result<Vec<_>>>()?
													)
												}
											))
										})
								)
								.collect::<Result<_>>()?
						})?)?
					)?;
				}
			}
		}

		filename if filename.ends_with(".sfx.wem") => {
			let (wwev, index) = filename
				.split('.')
				.next()
				.unwrap()
				.split_once('~')
				.ok_or_eyre("sfx.wem filename must follow format RuntimeID~index")?;

			let wwev = wwev.parse::<RuntimeID>()?;
			let index = index.parse::<usize>()?;

			for partition in &game_files.partitions {
				if let Ok(res_meta) = partition.get_resource_info(&wwev.as_u64().into())
					&& let Ok(res_data) = partition.read_resource(&wwev.as_u64().into())
				{
					let wwev_data = WwiseEvent::parse(GlacierGame::H3, &res_data, &res_meta.try_into()?)?;
					let id = wwev_data
						.non_streamed
						.get(index)
						.ok_or_else(|| eyre!("No such non-streamed audio object with index {index}"))?
						.wem_id;

					fs::rename(
						file_path,
						file_path.with_file_name(format!("{}~{}.sfx.wem", wwev.to_hash(), id))
					)?;
				}
			}
		}

		filename if filename.starts_with('0') => {
			upgrade_raw_file(game_files, file_path)?;
		}

		_ => {}
	}
}
