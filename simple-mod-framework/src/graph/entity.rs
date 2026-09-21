use std::sync::Arc;

use color_eyre::eyre::{Result, WrapErr, eyre};
use ecow::{EcoString, eco_format};
use glacier_bin1::{deserialize, ser::Bin1Serializer, serialize};
use glacier_commons::{game::GlacierGame, metadata::RuntimeID, rid};
use lazy_regex::regex_replace;
use quickentity_rs::{
	apply_patch,
	entity::{Entity, SubType},
	patch::Patch,
	variant::Variant
};
use serde::{Deserialize, Serialize};
use simple_mod_framework_core::{
	game::{NominalPartition, ResourceSpecifier},
	intentional_halt,
	utils::ResultExt
};
use simple_mod_framework_types::{PackageDefinitionEntity, VersionPlatform};
use tryvial::try_fn;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
	analysis::{ANALYSERS, Analyser},
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{Attribution, GraphOperation, RUNE_OPERATIONS, RuneOperation, rune_operation},
	register_operation,
	state::{Mutation, ResourceState, State},
	utils::ResultArtifacts,
	world::{Diagnostics, World}
};

const SHARED_BLUEPRINT_ENTITIES: [RuntimeID; 3] = [
	rid!("[assembly:/templates/geometrytemplatenocoll.template?/geomentity01.entitytemplate].pc_entityblueprint"),
	rid!("[assembly:/templates/geometrytemplaterigidbody.template?/geomentity01.entitytemplate].pc_entityblueprint"),
	rid!("[assembly:/templates/geometrytemplatestaticcoll.template?/geomentity01.entitytemplate].pc_entityblueprint")
];

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct ApplyQuickEntityPatch {
	#[rune(get, set)]
	pub specs: (ResourceSpecifier, ResourceSpecifier),

	#[rune(get, set)]
	pub data: Patch
}

register_operation!(ApplyQuickEntityPatch);

impl GraphOperation for ApplyQuickEntityPatch {
	fn warn_on_identical(&self) -> bool {
		false
	}

	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.specs.0.to_owned(), self.specs.1.to_owned()]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.specs.0.to_owned(), self.specs.1.to_owned()]
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

		let fac_states = state.get_resource_states(&self.specs.0).await?;
		let (fac_attr, fac_res) = fac_states.back().unwrap();
		let blu_states = state.get_resource_states(&self.specs.1).await?;
		let (blu_attr, blu_res) = blu_states.back().unwrap();

		let prev_states_artifacts = || {
			if fac_states.len() != blu_states.len() || fac_states.iter().zip(blu_states.iter()).any(|(a, b)| a.0 != b.0)
			{
				vec![("Patch being applied".into(), serde_json::to_vec(&self.data).unwrap())]
			} else {
				std::iter::once(("Patch being applied".into(), serde_json::to_vec(&self.data).unwrap()))
					.chain(
						fac_states
							.iter()
							.rev()
							.skip(1)
							.rev()
							.zip(blu_states.iter().rev().skip(1).rev())
							.map(|(fac, blu)| {
								(
									if let Some(attr) = fac.0.as_ref() {
										format!("Entity after {} - {}", attr.mod_id, attr.source)
									} else {
										"Vanilla entity".into()
									},
									serde_json::to_vec(&match state.game.version {
										GlacierGame::H1 => Entity::from_game(
											&deserialize::<glacier_bin1::game::h1::STemplateEntity>(&fac.1.data)
												.unwrap(),
											&fac.1.metadata,
											&deserialize::<glacier_bin1::game::h1::STemplateEntityBlueprint>(
												&blu.1.data
											)
											.unwrap(),
											&blu.1.metadata,
											true
										)
										.map_err(|x| eyre!("QuickEntity error: {:?}", x))
										.unwrap(),

										GlacierGame::H2 => Entity::from_game(
											&deserialize::<glacier_bin1::game::h2::STemplateEntityFactory>(&fac.1.data)
												.unwrap(),
											&fac.1.metadata,
											&deserialize::<glacier_bin1::game::h2::STemplateEntityBlueprint>(
												&blu.1.data
											)
											.unwrap(),
											&blu.1.metadata,
											true
										)
										.map_err(|x| eyre!("QuickEntity error: {:?}", x))
										.unwrap(),

										GlacierGame::H3 => Entity::from_game(
											&deserialize::<glacier_bin1::game::h3::STemplateEntityFactory>(&fac.1.data)
												.unwrap(),
											&fac.1.metadata,
											&deserialize::<glacier_bin1::game::h3::STemplateEntityBlueprint>(
												&blu.1.data
											)
											.unwrap(),
											&blu.1.metadata,
											true
										)
										.map_err(|x| eyre!("QuickEntity error: {:?}", x))
										.unwrap(),

										GlacierGame::FL => Entity::from_game(
											&deserialize::<glacier_bin1::game::fl::STemplateEntityFactory>(&fac.1.data)
												.unwrap(),
											&fac.1.metadata,
											&deserialize::<glacier_bin1::game::fl::STemplateEntityBlueprint>(
												&blu.1.data
											)
											.unwrap(),
											&blu.1.metadata,
											true
										)
										.map_err(|x| eyre!("QuickEntity error: {:?}", x))
										.unwrap()
									})
									.unwrap()
								)
							})
					)
					.collect()
			}
		};

		let last_state_binary_artifact = || {
			[
				(
					if let Some(fac_attr) = fac_attr {
						format!("Factory after {} - {}", fac_attr.mod_id, fac_attr.source)
					} else {
						"Vanilla factory".into()
					},
					fac_res.data.to_owned()
				),
				(
					if let Some(blu_attr) = blu_attr {
						format!("Blueprint after {} - {}", blu_attr.mod_id, blu_attr.source)
					} else {
						"Vanilla blueprint".into()
					},
					blu_res.data.to_owned()
				)
			]
		};

		let last_state_artifact = || {
			[(
				match (fac_attr.as_ref(), blu_attr.as_ref()) {
					(None, None) => "Vanilla entity".into(),
					(Some(fac_attr), Some(blu_attr)) if fac_attr == blu_attr => {
						format!("Entity after {} - {}", fac_attr.mod_id, fac_attr.source)
					}
					(Some(fac_attr), None) => format!(
						"Entity after factory modified by {} - {}, vanilla TBLU",
						fac_attr.mod_id, fac_attr.source
					),
					(None, Some(blu_attr)) => format!(
						"Entity after blueprint modified by {} - {}, vanilla TEMP",
						blu_attr.mod_id, blu_attr.source
					),
					(Some(fac_attr), Some(blu_attr)) => format!(
						"Entity after factory modified by {} - {}, blueprint modified by {} - {}",
						fac_attr.mod_id, fac_attr.source, blu_attr.mod_id, blu_attr.source
					)
				},
				serde_json::to_vec(&match state.game.version {
					GlacierGame::H1 => Entity::from_game(
						&deserialize::<glacier_bin1::game::h1::STemplateEntity>(&fac_res.data).unwrap(),
						&fac_res.metadata,
						&deserialize::<glacier_bin1::game::h1::STemplateEntityBlueprint>(&blu_res.data).unwrap(),
						&blu_res.metadata,
						true
					)
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.unwrap(),

					GlacierGame::H2 => Entity::from_game(
						&deserialize::<glacier_bin1::game::h2::STemplateEntityFactory>(&fac_res.data).unwrap(),
						&fac_res.metadata,
						&deserialize::<glacier_bin1::game::h2::STemplateEntityBlueprint>(&blu_res.data).unwrap(),
						&blu_res.metadata,
						true
					)
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.unwrap(),

					GlacierGame::H3 => Entity::from_game(
						&deserialize::<glacier_bin1::game::h3::STemplateEntityFactory>(&fac_res.data).unwrap(),
						&fac_res.metadata,
						&deserialize::<glacier_bin1::game::h3::STemplateEntityBlueprint>(&blu_res.data).unwrap(),
						&blu_res.metadata,
						true
					)
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.unwrap(),

					GlacierGame::FL => Entity::from_game(
						&deserialize::<glacier_bin1::game::fl::STemplateEntityFactory>(&fac_res.data).unwrap(),
						&fac_res.metadata,
						&deserialize::<glacier_bin1::game::fl::STemplateEntityBlueprint>(&blu_res.data).unwrap(),
						&blu_res.metadata,
						true
					)
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.unwrap()
				})
				.unwrap()
			)]
		};

		log::trace!(target: attribution.source.as_str(), "Converting factory/blueprint");

		let mut mutations = Vec::with_capacity(2);

		macro_rules! impl_game {
			($game:ident, $factory:ident) => {{
				let (fac, blu) = rayon::join(
					|| {
						deserialize::<glacier_bin1::game::$game::$factory>(&fac_res.data)
							.wrap_err("Couldn't deserialise factory")
					},
					|| {
						deserialize::<glacier_bin1::game::$game::STemplateEntityBlueprint>(&blu_res.data)
							.wrap_err("Couldn't deserialise blueprint")
					}
				);

				let (fac, blu) = (
					fac.with_artifacts(|| {
						prev_states_artifacts()
							.into_iter()
							.chain(last_state_binary_artifact())
							.collect()
					})?,
					blu.with_artifacts(|| {
						prev_states_artifacts()
							.into_iter()
							.chain(last_state_binary_artifact())
							.collect()
					})?
				);

				log::trace!(target: attribution.source.as_str(), "Converting entity");

				let mut entity = Entity::from_game(&fac, &fac_res.metadata, &blu, &blu_res.metadata, true)
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.with_artifacts(|| {
						prev_states_artifacts()
							.into_iter()
							.chain(last_state_binary_artifact())
							.collect()
					})?;

				drop(fac);
				drop(blu);

				log::trace!(target: attribution.source.as_str(), "Applying patch");

				let modified = apply_patch(&mut entity, self.data.to_owned(), |diagnostic| {
					let _ = state.world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::QuickEntityPatchDiagnostic {
							diagnostic: diagnostic.to_string()
						},
						target: attribution.to_diagnostic_target()
					});
				})
				.map_err(|x| eyre!("QuickEntity error: {:?}", x))
				.intentional()
				.with_artifacts(|| {
					prev_states_artifacts()
						.into_iter()
						.chain(last_state_artifact())
						.chain([(
							"Working copy of entity (some patches may have already been applied!)".into(),
							serde_json::to_vec(&entity).unwrap()
						)])
						.collect()
				})?;

				if !modified {
					state.world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::QuickEntityPatchNoEffect,
						target: attribution.to_diagnostic_target()
					})?;
				}

				if SHARED_BLUEPRINT_ENTITIES.contains(&self.specs.1.id) && entity.sub_entities.len() != 1 {
					intentional_halt!(
						attribution.source,
						if state.config.developer_mode {
							"Altering a geomentity01 shared-blueprint entity in an incompatible fashion causes game crashes \
							immediately upon loading. Consider creating a new entity based on the shared-blueprint entity \
							instead."
						} else {
							"Altering a geomentity01 shared-blueprint entity in an incompatible fashion causes game crashes \
							immediately upon loading. Contact the mod author to resolve this."
						}
					);
				}

				for sub_entity in entity.sub_entities.values() {
					if sub_entity.factory.resource
						== rid!("[assembly:/templates/ui/mapexportentities.template?/menumap.entitytemplate].pc_entitytype")
						&& let Some(res) = sub_entity.properties.get("m_pMetaDataResource")
						&& let Variant::Resource(_, Some(res)) = &res.value
					{
						mutations.push(Mutation::RegisterWorldMapMetadata { id: res.resource });
					}
				}

				log::trace!(target: attribution.source.as_str(), "Converting back to BIN1");

				let (fac, fac_meta, blu, blu_meta): (glacier_bin1::game::$game::$factory, _, _, _) = entity
					.to_game()
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.intentional()
					.with_artifacts(|| {
						prev_states_artifacts()
							.into_iter()
							.chain(last_state_artifact())
							.chain(std::iter::once((
								"Entity after patch".into(),
								serde_json::to_vec(&entity).unwrap()
							)))
							.collect()
					})?;

				log::trace!(target: attribution.source.as_str(), "Serialising");

				let (fac_data, blu_data) = rayon::join(
					|| {
						let after_patch_artifacts = || {
							prev_states_artifacts()
								.into_iter()
								.chain(last_state_artifact())
								.chain([
									("Entity after patch".into(), serde_json::to_vec(&entity).unwrap()),
									("Factory after patch".into(), serde_json::to_vec(&fac).unwrap())
								])
								.collect()
						};

						if state.game.version == GlacierGame::FL {
							Bin1Serializer::new().with_inline_arrays(false).serialize(&fac)
						} else {
							serialize(&fac)
						}
						.intentional()
						.with_artifacts(after_patch_artifacts)
					},
					|| {
						let after_patch_artifacts = || {
							prev_states_artifacts()
								.into_iter()
								.chain(last_state_artifact())
								.chain([
									("Entity after patch".into(), serde_json::to_vec(&entity).unwrap()),
									("Blueprint after patch".into(), serde_json::to_vec(&blu).unwrap())
								])
								.collect()
						};

						if state.game.version == GlacierGame::FL {
							Bin1Serializer::new().with_inline_arrays(false).serialize(&blu)
						} else {
							serialize(&blu)
						}
						.intentional()
						.with_artifacts(after_patch_artifacts)
					}
				);

				log::trace!(target: attribution.source.as_str(), "Finalising");

				mutations.push(Mutation::SetResourceValue {
					resource: self.specs.0.to_owned(),
					value: ResourceState {
						metadata: fac_meta,
						data: fac_data?
					}
				});
				mutations.push(Mutation::SetResourceValue {
					resource: self.specs.1,
					value: ResourceState {
						metadata: blu_meta,
						data: blu_data?
					}
				});

				if (entity.sub_type == SubType::Scene || entity.sub_type == SubType::Brick)
					&& let Some(path) = entity.factory.get_path()
				{
					mutations.push(Mutation::AddPackageDefinitionEntry {
						data: PackageDefinitionEntity {
							partition: eco_format!(
								"{}-{}",
								match state.game.version {
									GlacierGame::H1 => "h1",
									GlacierGame::H2 => "h2",
									GlacierGame::H3 => "h3",
									GlacierGame::FL => "fl"
								},
								*self.specs.0.partition
							)
							.try_into()
							.unwrap(),
							path: EcoString::from(regex_replace!(r"\.pc_([a-z_]+)$", &path, ".$1"))
								.try_into()
								.map_err(|e| eyre!("Invalid factory path: {e}"))?
						}
					})
				}
			}}
		}

		match state.game.version {
			GlacierGame::H1 => impl_game!(h1, STemplateEntity),
			GlacierGame::H2 => impl_game!(h2, STemplateEntityFactory),
			GlacierGame::H3 => impl_game!(h3, STemplateEntityFactory),
			GlacierGame::FL => impl_game!(fl, STemplateEntityFactory)
		}

		mutations
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_PATCH: Analyser = Analyser {
	file_type: "entity.patch.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: Patch = serde_json::from_slice(&file_contents)
			.wrap_err("Patch JSON was not valid QN patch")
			.intentional()?;

		let factory = game.infer_resource_specifier(data.factory)?.map_or_else(
			|| game.realise_resource_specifier(data.factory, NominalPartition(partition.to_owned())),
			|x| Ok(Some(x))
		)?;

		let blueprint = game.infer_resource_specifier(data.blueprint)?.map_or_else(
			|| game.realise_resource_specifier(data.blueprint, NominalPartition(partition.to_owned())),
			|x| Ok(Some(x))
		)?;

		if let Some(factory) = factory
			&& let Some(blueprint) = blueprint
		{
			Ok(vec![
				ApplyQuickEntityPatch {
					specs: (factory, blueprint),
					data
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
pub struct OverwriteEntity {
	#[rune(get, set)]
	pub specs: (ResourceSpecifier, ResourceSpecifier),

	#[rune(get, set)]
	pub data: Entity
}

register_operation!(OverwriteEntity);

impl GraphOperation for OverwriteEntity {
	fn warn_on_identical(&self) -> bool {
		false
	}

	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.specs.0.to_owned(), self.specs.1.to_owned()]
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
		if SHARED_BLUEPRINT_ENTITIES.contains(&self.specs.1.id) && self.data.sub_entities.len() != 1 {
			intentional_halt!(
				attribution.source,
				if state.config.developer_mode {
					"Altering a geomentity01 shared-blueprint entity in an incompatible fashion causes game crashes \
					 immediately upon loading. Consider creating a new entity based on the shared-blueprint entity \
					 instead."
				} else {
					"Altering a geomentity01 shared-blueprint entity in an incompatible fashion causes game crashes \
					 immediately upon loading. Contact the mod author to resolve this."
				}
			);
		}

		let mut mutations = Vec::with_capacity(2);

		macro_rules! impl_game {
			($game:ident, $factory:ident) => {{
				let (fac, fac_meta, blu, blu_meta): (glacier_bin1::game::$game::$factory, _, _, _) = self
					.data
					.to_game()
					.map_err(|x| eyre!("QuickEntity error: {:?}", x))
					.intentional()
					.with_artifacts(|| vec![("Entity being converted".into(), serde_json::to_vec(&self.data).unwrap())])?;

				if (self.data.sub_type == SubType::Scene || self.data.sub_type == SubType::Brick)
					&& let Some(path) = self.data.factory.get_path()
				{
					mutations.push(Mutation::AddPackageDefinitionEntry {
						data: PackageDefinitionEntity {
							partition: eco_format!(
								"{}-{}",
								match state.game.version {
									GlacierGame::H1 => "h1",
									GlacierGame::H2 => "h2",
									GlacierGame::H3 => "h3",
									GlacierGame::FL => "fl"
								},
								*self.specs.0.partition
							)
							.try_into()
							.unwrap(),
							path: EcoString::from(regex_replace!(r"\.pc_([a-z_]+)$", &path, ".$1"))
								.try_into()
								.map_err(|e| eyre!("Invalid factory path: {e}"))?
						}
					})
				}

				for sub_entity in self.data.sub_entities.values() {
					if sub_entity.factory.resource
						== rid!("[assembly:/templates/ui/mapexportentities.template?/menumap.entitytemplate].pc_entitytype")
						&& let Some(res) = sub_entity.properties.get("m_pMetaDataResource")
						&& let Variant::Resource(_, Some(res)) = &res.value
					{
						mutations.push(Mutation::RegisterWorldMapMetadata { id: res.resource });
					}
				}

				drop(self.data);

				log::trace!(target: attribution.source.as_str(), "Serialising");

				let (fac_data, blu_data) = rayon::join(
					|| {
						let artifacts = || vec![("Converted factory".into(), serde_json::to_vec(&fac).unwrap())];

						if state.game.version == GlacierGame::FL {
							Bin1Serializer::new().with_inline_arrays(false).serialize(&fac)
						} else {
							serialize(&fac)
						}
						.wrap_err("Couldn't serialise factory")
						.intentional()
						.with_artifacts(artifacts)
					},
					|| {
						let artifacts = || vec![("Converted blueprint".into(), serde_json::to_vec(&blu).unwrap())];

						if state.game.version == GlacierGame::FL {
							Bin1Serializer::new().with_inline_arrays(false).serialize(&blu)
						} else {
							serialize(&blu)
						}
						.wrap_err("Couldn't serialise blueprint")
						.intentional()
						.with_artifacts(artifacts)
					}
				);

				log::trace!(target: attribution.source.as_str(), "Finalising");

				mutations.extend([
					Mutation::SetResourceValue {
						resource: self.specs.0,
						value: ResourceState {
							metadata: fac_meta,
							data: fac_data?
						}
					},
					Mutation::SetResourceValue {
						resource: self.specs.1,
						value: ResourceState {
							metadata: blu_meta,
							data: blu_data?
						}
					}
				]);
			}}
		}

		match state.game.version {
			GlacierGame::H1 => impl_game!(h1, STemplateEntity),
			GlacierGame::H2 => impl_game!(h2, STemplateEntityFactory),
			GlacierGame::H3 => impl_game!(h3, STemplateEntityFactory),
			GlacierGame::FL => impl_game!(fl, STemplateEntityFactory)
		}

		mutations
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_ENTITY: Analyser = Analyser {
	file_type: "entity.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: Entity = serde_json::from_slice(&file_contents)
			.wrap_err("Entity JSON was not valid entity")
			.intentional()?;

		if let Some(factory) = game.realise_resource_specifier(data.factory, NominalPartition(partition.to_owned()))?
			&& let Some(blueprint) =
				game.realise_resource_specifier(data.blueprint, NominalPartition(partition.to_owned()))?
		{
			Ok(vec![
				OverwriteEntity {
					specs: (factory, blueprint),
					data
				}
				.into(),
			])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};
