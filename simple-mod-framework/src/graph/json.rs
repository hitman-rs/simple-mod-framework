use std::sync::Arc;

use color_eyre::eyre::{OptionExt, Result, WrapErr, bail, eyre};
use ecow::EcoString;
use glacier_bin1::types::resource::ZRuntimeResourceID;
use glacier_commons::{
	game::GlacierGame,
	metadata::{ReferenceFlags, ReferenceType, ResourceMetadata, ResourceReference, RuntimeID},
	resource_type, rid
};
use itertools::Itertools;
use rpkg_rs::resource::pdefs::{PartitionId, PartitionType};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str, json, to_string};
use simple_mod_framework_core::{
	game::{NominalPartition, REPO_ID, ResourceSpecifier, UNLOCKABLES_ID_FL, UNLOCKABLES_ID_WOA},
	resource_in,
	utils::ResultExt
};
use simple_mod_framework_types::{HashSet, VersionPlatform};
use tryvial::try_fn;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
	analysis::{ANALYSERS, Analyser},
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{Attribution, GraphOperation, RUNE_OPERATIONS, RuneOperation, rune_operation},
	register_operation,
	scripts::JsonValue,
	state::{Mutation, ResourceState, State},
	world::{Diagnostics, World}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct ApplyJSONMergePatch {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub data: JsonValue
}

register_operation!(ApplyJSONMergePatch);

impl GraphOperation for ApplyJSONMergePatch {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.spec.to_owned()]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.spec.to_owned()]
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
			.expect("Couldn't serialise ApplyQuickEntityPatch")
		)
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		if self.spec.id == REPO_ID {
			let repo = state.get_resource(&self.spec).await?;

			let current: Value = serde_json::from_slice(&repo.data).wrap_err("Couldn't parse repository as JSON")?;

			log::trace!(target: attribution.source.as_str(), "Patching repository");

			let mut new = Value::Object(
				current
					.as_array()
					.ok_or_eyre("Repository was not array")?
					.iter()
					.map(|x| {
						Ok((
							x.as_object()
								.ok_or_eyre("Repository item was not object")?
								.get("ID_")
								.ok_or_eyre("Repository item had no ID_ key")?
								.as_str()
								.ok_or_eyre("Repository item ID_ key was not string")?
								.to_owned(),
							x.to_owned()
						))
					})
					.collect::<Result<_>>()?
			);

			json_patch::merge(&mut new, &self.data);

			let new_repository = Value::Array(
				new.as_object()
					.unwrap()
					.iter()
					.map(|(x, y)| {
						Ok({
							let mut z = y.to_owned();

							if !z
								.as_object()
								.ok_or_eyre("Repository item was not object")?
								.contains_key("ID_")
							{
								z["ID_"] = Value::String(x.to_owned());
							}

							z
						})
					})
					.collect::<Result<_>>()?
			);

			let edited_keys = self
				.data
				.as_object()
				.ok_or_eyre("Repository patch was not object")
				.intentional()?
				.keys()
				.collect_vec();

			let mut metadata = repo.metadata.to_owned();

			let existing_references = metadata.references.iter().map(|x| x.resource).collect::<HashSet<_>>();

			for entry in new_repository.as_array().unwrap() {
				let entry = entry.as_object().unwrap();

				if edited_keys.contains(&&entry["ID_"].as_str().ok_or_eyre("ID_ key was not string")?.to_owned()) {
					for key in [
						"Runtime",
						"ImpactEffect",
						"DeathImpactEffect",
						"AmmoImpactEffect",
						"AmmoInFlightEffect"
					] {
						if let Some(dep) = entry.get(key) {
							let dep = dep
								.as_str()
								.ok_or_else(|| eyre!("{key} key was not string"))?
								.parse::<u64>()
								.wrap_err_with(|| format!("{key} key was not valid number"))?;

							if dep != u64::MAX {
								let depend = ResourceReference {
									resource: RuntimeID::try_from(dep)
										.wrap_err_with(|| format!("{key} key was not valid RuntimeID"))?,
									flags: ReferenceFlags {
										reference_type: ReferenceType::Weak,
										acquired: false,
										language_code: 0x1F
									}
								};

								if !existing_references.contains(&depend.resource) {
									metadata.references.push(depend);
								}
							}
						}
					}

					if let Some(image) = entry.get("Image") {
						let resource = RuntimeID::from_path(&format!(
							"[assembly:/_pro/online/default/cloudstorage/resources/{}].{}gfx",
							image.as_str().ok_or_eyre("Image key was not string")?.to_lowercase(),
							if state.game.version != GlacierGame::FL {
								"pc_"
							} else {
								""
							}
						));

						if state.game.vanilla_resources.contains_key(&resource)
							|| state.deployment.all_relevant_resources.contains_key(&resource)
						{
							let depend = ResourceReference {
								resource,
								flags: ReferenceFlags {
									reference_type: ReferenceType::Weak,
									acquired: false,
									language_code: 0x1F
								}
							};

							if !existing_references.contains(&depend.resource) {
								metadata.references.push(depend);
							}
						}
					}
				}
			}

			log::trace!(target: attribution.source.as_str(), "Finalising");

			vec![Mutation::SetResourceValue {
				resource: self.spec.to_owned(),
				value: ResourceState {
					metadata,
					data: serde_json::to_vec(&new_repository)?
				}
			}]
		} else if self.spec.id == UNLOCKABLES_ID_WOA || self.spec.id == UNLOCKABLES_ID_FL {
			let ores = state.get_resource(&self.spec).await?;

			let current: Value = from_str(&glacier_bin1::deserialize::<EcoString>(&ores.data)?)?;

			log::trace!(target: attribution.source.as_str(), "Patching unlockables");

			let mut new = Value::Object(
				current
					.as_array()
					.ok_or_eyre("Unlockables ORES was not array")?
					.iter()
					.map(|x| {
						Ok((
							x.as_object()
								.ok_or_eyre("Unlockables item was not object")?
								.get("Id")
								.ok_or_eyre("Unlockables item had no Id key")?
								.as_str()
								.ok_or_eyre("Unlockables item Id key was not string")?
								.to_owned(),
							x.to_owned()
						))
					})
					.collect::<Result<_>>()?
			);

			json_patch::merge(&mut new, &self.data);

			let new_unlockables = Value::Array(
				new.as_object()
					.unwrap()
					.iter()
					.map(|(x, y)| {
						Ok({
							let mut z = y.to_owned();

							if !z
								.as_object()
								.ok_or_eyre("Unlockables item was not object")?
								.contains_key("Id")
							{
								z["Id"] = Value::String(x.to_owned());
							}

							z
						})
					})
					.collect::<Result<_>>()?
			);

			log::trace!(target: attribution.source.as_str(), "Finalising");

			vec![Mutation::SetResourceValue {
				resource: self.spec.to_owned(),
				value: ResourceState {
					metadata: ores.metadata.to_owned(),
					data: glacier_bin1::serialize(&EcoString::from(to_string(&new_unlockables)?))?
				}
			}]
		} else {
			bail!("Merge patch constructed with unsupported resource ID")
		}
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_REPOSITORY: Analyser = Analyser {
	file_type: "repository.json",
	analyse: |_, game, _, _, _, _, file_contents| {
		let data: Value = serde_json::from_slice(&file_contents)
			.wrap_err("Repository JSON was not valid JSON")
			.intentional()?;

		Ok(vec![
			ApplyJSONMergePatch {
				spec: ResourceSpecifier {
					id: REPO_ID,
					partition: NominalPartition("super".into()).real_candidates(game).unwrap()[0].to_owned()
				},
				data: data.into()
			}
			.into(),
		])
	}
};

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_UNLOCKABLES: Analyser = Analyser {
	file_type: "unlockables.json",
	analyse: |_, game, _, _, _, _, file_contents| {
		let data: Value = serde_json::from_slice(&file_contents)
			.wrap_err("Unlockables JSON was not valid JSON")
			.intentional()?;

		Ok(vec![
			ApplyJSONMergePatch {
				spec: ResourceSpecifier {
					id: if game.version == GlacierGame::FL {
						UNLOCKABLES_ID_FL
					} else {
						UNLOCKABLES_ID_WOA
					},
					partition: NominalPartition("super".into()).real_candidates(game).unwrap()[0].to_owned()
				},
				data: data.into()
			}
			.into(),
		])
	}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct ApplyJSONPatch {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub data: JsonValue
}

register_operation!(ApplyJSONPatch);

impl GraphOperation for ApplyJSONPatch {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.spec.to_owned()]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.spec.to_owned()]
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
		let ResourceState {
			metadata: mut res_meta,
			data: res_data
		} = state.get_resource(&self.spec).await?.to_owned();

		if res_meta.resource_type == "REPO" {
			log::trace!(target: attribution.source.as_str(), "Acquiring resources");

			let current_data: Value =
				serde_json::from_slice(&res_data).wrap_err("Couldn't parse repository as JSON")?;

			log::trace!(target: attribution.source.as_str(), "Patching repository");

			let mut new = Value::Object(
				current_data
					.as_array()
					.ok_or_eyre("Repository was not array")?
					.iter()
					.map(|x| {
						Ok((
							x.as_object()
								.ok_or_eyre("Repository item was not object")?
								.get("ID_")
								.ok_or_eyre("Repository item had no ID_ key")?
								.as_str()
								.ok_or_eyre("Repository item ID_ key was not string")?
								.to_owned(),
							x.to_owned()
						))
					})
					.collect::<Result<_>>()?
			);

			json_patch::patch(
				&mut new,
				&serde_json::from_value::<Vec<_>>(self.data.to_owned().into()).wrap_err("JSON patch was invalid")?
			)
			.wrap_err("Couldn't apply JSON patch")?;

			let new_repository = Value::Array(
				new.as_object()
					.unwrap()
					.iter()
					.map(|(x, y)| {
						Ok({
							let mut z = y.to_owned();

							if !z
								.as_object()
								.ok_or_eyre("Repository item was not object")?
								.contains_key("ID_")
							{
								z["ID_"] = Value::String(x.to_owned());
							}

							z
						})
					})
					.collect::<Result<_>>()?
			);

			let existing_references = res_meta.references.iter().map(|x| x.resource).collect::<HashSet<_>>();

			for entry in new_repository.as_array().unwrap() {
				if current_data
					.as_array()
					.ok_or_eyre("Repository was not array")?
					.iter()
					.find(|x| x["ID_"].as_str().unwrap() == entry["ID_"].as_str().unwrap())
					!= Some(entry)
				{
					let entry = entry.as_object().unwrap();

					for key in [
						"Runtime",
						"ImpactEffect",
						"DeathImpactEffect",
						"AmmoImpactEffect",
						"AmmoInFlightEffect"
					] {
						if let Some(dep) = entry.get(key) {
							let dep = dep
								.as_str()
								.ok_or_else(|| eyre!("{key} key was not string"))?
								.parse::<u64>()
								.wrap_err_with(|| format!("{key} key was not valid number"))?;

							if dep != u64::MAX {
								let depend = ResourceReference {
									resource: RuntimeID::try_from(dep)
										.wrap_err_with(|| format!("{key} key was not valid RuntimeID"))?,
									flags: ReferenceFlags {
										reference_type: ReferenceType::Weak,
										acquired: false,
										language_code: 0x1F
									}
								};

								if !existing_references.contains(&depend.resource) {
									res_meta.references.push(depend);
								}
							}
						}
					}

					if let Some(image) = entry.get("Image") {
						let resource = RuntimeID::from_path(&format!(
							"[assembly:/_pro/online/default/cloudstorage/resources/{}].{}gfx",
							image.as_str().ok_or_eyre("Image key was not string")?.to_lowercase(),
							if state.game.version != GlacierGame::FL {
								"pc_"
							} else {
								""
							}
						));

						if state.game.vanilla_resources.contains_key(&resource)
							|| state.deployment.all_relevant_resources.contains_key(&resource)
						{
							let depend = ResourceReference {
								resource,
								flags: ReferenceFlags {
									reference_type: ReferenceType::Weak,
									acquired: false,
									language_code: 0x1F
								}
							};

							if !existing_references.contains(&depend.resource) {
								res_meta.references.push(depend);
							}
						}
					}
				}
			}

			log::trace!(target: attribution.source.as_str(), "Finalising");

			vec![Mutation::SetResourceValue {
				resource: self.spec.to_owned(),
				value: ResourceState {
					metadata: res_meta,
					data: serde_json::to_vec(&new_repository)?
				}
			}]
		} else if res_meta.resource_type == "ORES" {
			log::trace!(target: attribution.source.as_str(), "Acquiring resources");

			let current_data: Value = from_str(&glacier_bin1::deserialize::<EcoString>(&res_data)?)?;

			log::trace!(target: attribution.source.as_str(), "Patching unlockables");

			let mut new = Value::Object(
				current_data
					.as_array()
					.ok_or_eyre("Unlockables ORES was not array")?
					.iter()
					.map(|x| {
						Ok((
							x.as_object()
								.ok_or_eyre("Unlockables item was not object")?
								.get("Id")
								.ok_or_eyre("Unlockables item had no Id key")?
								.as_str()
								.ok_or_eyre("Unlockables item Id key was not string")?
								.to_owned(),
							x.to_owned()
						))
					})
					.collect::<Result<_>>()?
			);

			json_patch::patch(
				&mut new,
				&serde_json::from_value::<Vec<_>>(self.data.into()).wrap_err("JSON patch was invalid")?
			)
			.wrap_err("Couldn't apply JSON patch")?;

			let new_unlockables = Value::Array(
				new.as_object()
					.unwrap()
					.iter()
					.map(|(x, y)| {
						Ok({
							let mut z = y.to_owned();

							if !z
								.as_object()
								.ok_or_eyre("Unlockables item was not object")?
								.contains_key("Id")
							{
								z["Id"] = Value::String(x.to_owned());
							}

							z
						})
					})
					.collect::<Result<_>>()?
			);

			log::trace!(target: attribution.source.as_str(), "Finalising");

			vec![Mutation::SetResourceValue {
				resource: self.spec.to_owned(),
				value: ResourceState {
					metadata: res_meta,
					data: glacier_bin1::serialize(&EcoString::from(to_string(&new_unlockables)?))?
				}
			}]
		} else {
			let mut to_patch = serde_json::from_slice(&res_data).wrap_err("Data to patch wasn't valid JSON")?;

			log::trace!(target: attribution.source.as_str(), "Patching JSON");

			json_patch::patch(
				&mut to_patch,
				&serde_json::from_value::<Vec<_>>(self.data.into()).wrap_err("JSON patch was invalid")?
			)
			.wrap_err("Couldn't apply JSON patch")?;

			log::trace!(target: attribution.source.as_str(), "Finalising");

			vec![Mutation::SetResourceValue {
				resource: self.spec.to_owned(),
				value: ResourceState {
					metadata: res_meta,
					data: serde_json::to_vec(&to_patch)?
				}
			}]
		}
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_JSONPATCH: Analyser = Analyser {
	file_type: "JSON.patch.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: Value = serde_json::from_slice(&file_contents)
			.wrap_err("Patch JSON was not valid JSON")
			.intentional()?;

		if let Some(spec) = game.realise_resource_specifier(
			serde_json::from_value::<RuntimeID>(
				data.get("id")
					.ok_or_eyre("Patch JSON had no id key")
					.intentional()?
					.to_owned()
			)
			.wrap_err("Patch JSON id was not valid")?,
			NominalPartition(partition.to_owned())
		)? {
			Ok(vec![
				ApplyJSONPatch {
					spec,
					data: data
						.get("patch")
						.ok_or_eyre("Patch JSON had no patch")
						.intentional()?
						.to_owned()
						.into()
				}
				.into(),
			])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

const CONTRACTS_ORES_WOA: &str = "[assembly:/_pro/online/default/offlineconfig/config.contracts].pc_contracts";

const CONTRACTS_ORES_FL: &str = "[assembly:/_knt/online/default/offlineconfig/config.contracts].contracts";

fn contract_resource(game: GlacierGame, id: &str) -> RuntimeID {
	if game == GlacierGame::FL {
		RuntimeID::from_path(&format!(
			"[assembly:/_pro/online/default/contracts/seed/missions/smf_{}.contract.json]({}).json",
			id, UNLOCKABLES_ID_FL
		))
	} else {
		RuntimeID::from_path(&format!(
			"[assembly:/_pro/online/default/contracts/seed/missions/smf_{}.contract.json]({}).pc_json",
			id, UNLOCKABLES_ID_WOA
		))
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct AddContractToORES {
	#[rune(get, set)]
	pub id: String
}

register_operation!(AddContractToORES);

impl GraphOperation for AddContractToORES {
	fn get_required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(CONTRACTS_ORES_FL, "super", platform)
		} else {
			resource_in!(CONTRACTS_ORES_WOA, "super", platform)
		}]
	}

	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(CONTRACTS_ORES_FL, "super", platform)
		} else {
			resource_in!(CONTRACTS_ORES_WOA, "super", platform)
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

	fn warn_on_identical(&self) -> bool {
		false
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		log::trace!(target: attribution.source.as_str(), "Acquiring resources");

		let ores = &state
			.get_resource(&if state.game.version == GlacierGame::FL {
				resource_in!(CONTRACTS_ORES_FL, "super", &*state.game)
			} else {
				resource_in!(CONTRACTS_ORES_WOA, "super", &*state.game)
			})
			.await?;

		macro_rules! impl_game {
			($game:ident) => {{
				let mut ores_data = glacier_bin1::deserialize::<Vec<glacier_bin1::game::$game::SContractConfigResourceEntry>>(&ores.data)?;

				log::trace!(target: attribution.source.as_str(), "Handling contract");

				let hash = match ores_data.iter().find(|x| x.id == self.id) {
					Some(x) => x.contract_rid.as_u64().try_into()?,

					_ => contract_resource(state.game.version, &self.id)
				};

				ores_data.push(glacier_bin1::game::$game::SContractConfigResourceEntry {
					id: self.id.into(),
					contract_rid: ZRuntimeResourceID::from_u64(state.game.to_rrid_u64(hash))
				});

				log::trace!(target: attribution.source.as_str(), "Finalising");

				vec![Mutation::SetResourceValue {
					resource: if state.game.version == GlacierGame::FL {
						resource_in!(CONTRACTS_ORES_FL, "super", &*state.game)
					} else {
						resource_in!(CONTRACTS_ORES_WOA, "super", &*state.game)
					},
					value: ResourceState {
						metadata: {
							let mut current = ores.metadata.to_owned();

							let x = ResourceReference {
								resource: hash,
								flags: ReferenceFlags {
									reference_type: ReferenceType::Weak,
									acquired: false,
									language_code: 0x1F
								}
							};

							if !current.references.contains(&x) {
								current.references.push(x);
							}

							current
						},
						data: glacier_bin1::serialize(&ores_data)?
					}
				}]
			}}
		}

		match state.game.version {
			GlacierGame::H1 => impl_game!(h1),
			GlacierGame::H2 => impl_game!(h2),
			GlacierGame::H3 => impl_game!(h3),
			GlacierGame::FL => impl_game!(fl)
		}
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct WriteContractJSON {
	#[rune(get, set)]
	pub id: RuntimeID,

	#[rune(get, set)]
	pub contract: JsonValue
}

register_operation!(WriteContractJSON);

impl GraphOperation for WriteContractJSON {
	fn should_cache(&self) -> bool {
		false
	}

	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![ResourceSpecifier {
			id: self.id,
			partition: NominalPartition("super".into()).real_candidates(platform).unwrap()[0].to_owned()
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
	async fn evaluate(self, _: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		vec![
			Mutation::SetResourceValue {
				resource: ResourceSpecifier {
					id: self.id,
					partition: NominalPartition("super".into()).real_candidates(&*state.game).unwrap()[0].to_owned()
				},
				value: ResourceState {
					metadata: ResourceMetadata {
						id: self.id,
						resource_type: resource_type!("JSON"),
						compressed: ResourceMetadata::infer_compressed(resource_type!("JSON")),
						scrambled: ResourceMetadata::infer_scrambled(resource_type!("JSON")),
						references: vec![]
					},
					data: serde_json::to_vec(&self.contract)?
				}
			},
			Mutation::AddServerSideContract {
				contract: self.contract["Metadata"]["Id"].as_str().unwrap().into(),
				data: self.contract
			},
		]
	}
}

const MISSION_CONFIG: &str = "[assembly:/_pro/online/default/cloudstorage/resources/missionconfig.json].pc_json";

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct AddContractToDestinations {
	#[rune(get, set)]
	pub id: String,

	#[rune(get, set)]
	pub place_before: Option<String>,

	#[rune(get, set)]
	pub place_after: Option<String>,

	#[rune(get, set)]
	pub narrative_context: String
}

register_operation!(AddContractToDestinations);

impl GraphOperation for AddContractToDestinations {
	fn get_required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![resource_in!(MISSION_CONFIG, "super", platform)]
	}

	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![resource_in!(MISSION_CONFIG, "super", platform)]
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
		let missionconfig = state
			.get_resource(&resource_in!(MISSION_CONFIG, "super", &*state.game))
			.await?;

		let mut destinations =
			serde_json::from_slice::<Value>(&missionconfig.data).wrap_err("Couldn't parse destinations as JSON")?;

		let children = destinations
			.get_mut("Root")
			.ok_or_eyre("No Root on missionconfig registry")?
			.get_mut("Children")
			.ok_or_eyre("No Children on missionconfig Root")?
			.as_array_mut()
			.ok_or_eyre("missionconfig Root.Children was not an array")?;

		if let Some(before) = self.place_before {
			let before = before.into();
			let before_idx = children.iter().position(|x| x.get("Id") == Some(&before));

			if let Some(before_idx) = before_idx {
				children.insert(
					before_idx,
					json!({
						"Id": self.id,
						"_comment": "Automatically added by SMF.",
						"NarrativeContext": self.narrative_context,
						"Meta": {
							"Ui": {
								"Row": 3,
								"Col": 5
							}
						}
					})
				);
			} else {
				state.world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::DestinationsItemNotFoundBefore {
						identifier: before.as_str().unwrap().to_string()
					},
					target: attribution.to_diagnostic_target()
				})?;

				children.push(json!({
					"Id": self.id,
					"_comment": "Automatically added by SMF.",
					"NarrativeContext": self.narrative_context,
					"Meta": {
						"Ui": {
							"Row": 3,
							"Col": 5
						}
					}
				}));
			}
		} else if let Some(after) = self.place_after {
			let after = after.into();
			let after_idx = children.iter().position(|x| x.get("Id") == Some(&after));

			if let Some(after_idx) = after_idx {
				children.insert(
					after_idx + 1,
					json!({
						"Id": self.id,
						"_comment": "Automatically added by SMF.",
						"NarrativeContext": self.narrative_context,
						"Meta": {
							"Ui": {
								"Row": 3,
								"Col": 5
							}
						}
					})
				);
			} else {
				state.world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::DestinationsItemNotFoundAfter {
						identifier: after.as_str().unwrap().to_string()
					},
					target: attribution.to_diagnostic_target()
				})?;

				children.push(json!({
					"Id": self.id,
					"_comment": "Automatically added by SMF.",
					"NarrativeContext": self.narrative_context,
					"Meta": {
						"Ui": {
							"Row": 3,
							"Col": 5
						}
					}
				}));
			}
		} else {
			children.push(json!({
				"Id": self.id,
				"_comment": "Automatically added by SMF.",
				"NarrativeContext": self.narrative_context,
				"Meta": {
					"Ui": {
						"Row": 3,
						"Col": 5
					}
				}
			}));
		}

		vec![Mutation::SetResourceValue {
			resource: resource_in!(MISSION_CONFIG, "super", &*state.game),
			value: ResourceState {
				metadata: missionconfig.metadata.to_owned(),
				data: serde_json::to_vec(&destinations)?
			}
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_CONTRACT: Analyser = Analyser {
	file_type: "contract.json",
	analyse: |_, game, _, _, _, _, file_contents| {
		let contract: Value = serde_json::from_slice(&file_contents)
			.wrap_err("Contract JSON was not valid JSON")
			.intentional()?;

		let id = contract
			.get("Metadata")
			.ok_or_eyre("Contract had no Metadata key")
			.intentional()?
			.get("Id")
			.ok_or_eyre("Contract metadata had no Id key")
			.intentional()?
			.as_str()
			.ok_or_eyre("Contract ID was not string")
			.intentional()?
			.to_owned();

		let ores = game.game_files.read_resource_from(
			PartitionId {
				index: 0,
				part_type: PartitionType::Standard
			},
			game.to_rrid(if game.version == GlacierGame::FL {
				rid!(CONTRACTS_ORES_FL)
			} else {
				rid!(CONTRACTS_ORES_WOA)
			})
		)?;

		macro_rules! impl_game {
			($game:ident) => {
				match glacier_bin1::deserialize::<Vec<glacier_bin1::game::$game::SContractConfigResourceEntry>>(&ores)?
					.into_iter()
					.find(|x| x.id == id)
				{
					Some(x) => x.contract_rid.as_u64().try_into()?,

					// If not vanilla, assume contract is added by SMF
					_ => contract_resource(game.version, &id)
				}
			};
		}

		let hash = match game.version {
			GlacierGame::H1 => impl_game!(h1),
			GlacierGame::H2 => impl_game!(h2),
			GlacierGame::H3 => impl_game!(h3),
			GlacierGame::FL => impl_game!(fl)
		};

		let mut operations = vec![AddContractToORES { id: id.to_owned() }.into()];

		if let Some(smf_integration) = contract.get("SMF") {
			let smf_integration = smf_integration
				.as_object()
				.ok_or_eyre("SMF key was not an object")
				.intentional()?;

			if let Some(destinations) = smf_integration.get("destinations") {
				let destinations = destinations
					.as_object()
					.ok_or_eyre("SMF destinations configuration was not an object")
					.intentional()?;

				if let Some(add_to_destinations) = destinations.get("addToDestinations") {
					let add_to_destinations = add_to_destinations
						.as_bool()
						.ok_or_eyre("addToDestinations was not a boolean")
						.intentional()?;

					if add_to_destinations {
						operations.push(
							AddContractToDestinations {
								id,
								place_before: destinations
									.get("placeBefore")
									.map(|x| {
										color_eyre::eyre::Ok(
											x.as_str()
												.ok_or_eyre("placeBefore was not a string")
												.intentional()?
												.to_owned()
										)
									})
									.transpose()?,
								place_after: destinations
									.get("placeAfter")
									.map(|x| {
										color_eyre::eyre::Ok(
											x.as_str()
												.ok_or_eyre("placeAfter was not a string")
												.intentional()?
												.to_owned()
										)
									})
									.transpose()?,
								narrative_context: destinations
									.get("narrativeContext")
									.map(|x| {
										color_eyre::eyre::Ok(
											x.as_str()
												.ok_or_eyre("narrativeContext was not a string")
												.intentional()?
												.to_owned()
										)
									})
									.transpose()?
									.unwrap_or_else(|| "Mission".into())
							}
							.into()
						);
					}
				}
			}
		}

		operations.push(
			WriteContractJSON {
				id: hash,
				contract: contract.into()
			}
			.into()
		);

		Ok(operations)
	}
};

const BLOBS_ORES_WOA: &str = "[assembly:/_pro/online/default/offlineconfig/config.blobs].pc_blobs";

const BLOBS_ORES_FL: &str = "[assembly:/_knt/online/default/offlineconfig/config.blobs].blobs";

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct AddBlobsToORES {
	#[rune(get, set)]
	pub blobs: Vec<(RuntimeID, String)>
}

register_operation!(AddBlobsToORES);

impl GraphOperation for AddBlobsToORES {
	fn get_required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(BLOBS_ORES_FL, "super", platform)
		} else {
			resource_in!(BLOBS_ORES_WOA, "super", platform)
		}]
	}

	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![if platform.version == GlacierGame::FL {
			resource_in!(BLOBS_ORES_FL, "super", platform)
		} else {
			resource_in!(BLOBS_ORES_WOA, "super", platform)
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

	fn warn_on_identical(&self) -> bool {
		false
	}

	#[try_fn]
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		log::trace!(target: attribution.source.as_str(), "Acquiring resources");

		let ores = state
			.get_resource(&if state.game.version == GlacierGame::FL {
				resource_in!(BLOBS_ORES_FL, "super", &*state.game)
			} else {
				resource_in!(BLOBS_ORES_WOA, "super", &*state.game)
			})
			.await?;

		macro_rules! impl_game {
			(fl) => {{
				let mut ores_data = glacier_bin1::deserialize::<Vec<glacier_bin1::game::fl::SBlobsConfigResourceEntry>>(&ores.data)?;

				log::trace!(target: attribution.source.as_str(), "Handling blobs");

				let mut metadata = ores.metadata.to_owned();

				let existing_references = metadata.references.iter().map(|x| x.resource).collect::<HashSet<_>>();

				metadata.references.extend(
					self.blobs
						.iter()
						.filter(|(id, _)| !existing_references.contains(id))
						.map(|(id, _)| ResourceReference {
							resource: *id,
							flags: ReferenceFlags {
								reference_type: ReferenceType::Weak,
								acquired: false,
								language_code: 0x1F
							}
						})
				);

				ores_data.extend(
					self.blobs.into_iter()
						.filter(|(id, _)| !existing_references.contains(id))
						.map(|(id, path)| glacier_bin1::game::fl::SBlobsConfigResourceEntry {
							id: path.into(),
							blob_rid: ZRuntimeResourceID::from_u64(state.game.to_rrid_u64(id)),
							parameter: "".into(), // TODO
							sub_ids: vec![]
						})
				);

				log::trace!(target: attribution.source.as_str(), "Finalising");

				let data = glacier_bin1::serialize(&ores_data)?;

				vec![Mutation::SetResourceValue {
					resource: if state.game.version == GlacierGame::FL {
						resource_in!(BLOBS_ORES_FL, "super", &*state.game)
					} else {
						resource_in!(BLOBS_ORES_WOA, "super", &*state.game)
					},
					value: ResourceState { metadata, data }
				}]
			}};

			($game:ident) => {{
				let mut ores_data = glacier_bin1::deserialize::<Vec<glacier_bin1::game::$game::SBlobsConfigResourceEntry>>(&ores.data)?;

				log::trace!(target: attribution.source.as_str(), "Handling blobs");

				let mut metadata = ores.metadata.to_owned();

				let existing_references = metadata.references.iter().map(|x| x.resource).collect::<HashSet<_>>();

				metadata.references.extend(
					self.blobs
						.iter()
						.filter(|(id, _)| !existing_references.contains(id))
						.map(|(id, _)| ResourceReference {
							resource: *id,
							flags: ReferenceFlags {
								reference_type: ReferenceType::Weak,
								acquired: false,
								language_code: 0x1F
							}
						})
				);

				ores_data.extend(
					self.blobs.into_iter()
						.filter(|(id, _)| !existing_references.contains(id))
						.map(|(id, path)| glacier_bin1::game::$game::SBlobsConfigResourceEntry {
							id: path.into(),
							blob_rid: ZRuntimeResourceID::from_u64(state.game.to_rrid_u64(id))
						})
				);

				log::trace!(target: attribution.source.as_str(), "Finalising");

				let data = glacier_bin1::serialize(&ores_data)?;

				vec![Mutation::SetResourceValue {
					resource: if state.game.version == GlacierGame::FL {
						resource_in!(BLOBS_ORES_FL, "super", &*state.game)
					} else {
						resource_in!(BLOBS_ORES_WOA, "super", &*state.game)
					},
					value: ResourceState { metadata, data }
				}]
			}}
		}

		match state.game.version {
			GlacierGame::H1 => impl_game!(h1),
			GlacierGame::H2 => impl_game!(h2),
			GlacierGame::H3 => impl_game!(h3),
			GlacierGame::FL => impl_game!(fl)
		}
	}
}
