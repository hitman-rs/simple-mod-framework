use std::{hash::Hash, io::Cursor, str::FromStr, sync::Arc};

use color_eyre::eyre::{OptionExt, Result, WrapErr, bail, eyre};
use ecow::EcoString;
use glacier_commons::{
	game::GlacierGame,
	metadata::{ReferenceFlags, ReferenceType, ResourceMetadata, ResourceReference, RuntimeID},
	resource_type
};
use glacier_formats::{
	material::{MaterialEntity, MaterialInstance},
	sdef::SoundDefinitions,
	texture::{InterpretAs, RenderFormat, TextureMetadata, TextureType},
	wwev::WwiseEvent
};
use glacier_texture::pack::TextureMapBuilder;
use rkyv::rancor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_mod_framework_core::{
	game::{NominalPartition, ResourceSpecifier},
	utils::ResultExt
};
use simple_mod_framework_types::{PackageDefinitionEntity, VersionPlatform};
use tryvial::try_fn;
use velcro::vec;
use xxhash_rust::xxh3::{Xxh3Default, xxh3_64};

use crate::{
	analysis::{ANALYSERS, Analyser},
	graph::{Attribution, GraphOperation, RUNE_OPERATIONS, RuneOperation, rune_operation},
	register_operation,
	state::{Mutation, ResourceState, State},
	world::World
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverwriteMaterial {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub data: MaterialInstance
}

register_operation!(OverwriteMaterial);

impl GraphOperation for OverwriteMaterial {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
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
	async fn evaluate(self, _: Attribution, _: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		let (data, metadata) = self
			.data
			.generate()
			.wrap_err("Couldn't generate material data")
			.intentional()?;

		vec![Mutation::SetResourceValue {
			resource: self.spec,
			value: ResourceState { metadata, data }
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_MATERIAL: Analyser = Analyser {
	file_type: "material.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: MaterialInstance = serde_json::from_slice(&file_contents)
			.wrap_err("Material JSON was not valid")
			.intentional()?;

		if let Some(spec) = game.realise_resource_specifier(data.id, NominalPartition(partition.to_owned()))? {
			Ok(vec![OverwriteMaterial { spec, data }.into()])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverwriteMaterialEntity {
	#[rune(get, set)]
	pub specs: (ResourceSpecifier, ResourceSpecifier),

	#[rune(get, set)]
	pub data: MaterialEntity
}

register_operation!(OverwriteMaterialEntity);

impl GraphOperation for OverwriteMaterialEntity {
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
	async fn evaluate(self, _: Attribution, _: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		let ((data1, metadata1), (data2, metadata2)) = self
			.data
			.generate()
			.wrap_err("Couldn't generate material entity data")
			.intentional()?;

		vec![
			Mutation::SetResourceValue {
				resource: self.specs.0,
				value: ResourceState {
					metadata: metadata1,
					data: data1
				}
			},
			Mutation::SetResourceValue {
				resource: self.specs.1,
				value: ResourceState {
					metadata: metadata2,
					data: data2
				}
			},
		]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_MATERIAL_ENTITY: Analyser = Analyser {
	file_type: "material.entity.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: MaterialEntity = serde_json::from_slice(&file_contents)
			.wrap_err("Material entity JSON was not valid")
			.intentional()?;

		if let Some(factory) = game.realise_resource_specifier(data.factory, NominalPartition(partition.to_owned()))?
			&& let Some(blueprint) =
				game.realise_resource_specifier(data.blueprint, NominalPartition(partition.to_owned()))?
		{
			Ok(vec![
				OverwriteMaterialEntity {
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

#[derive(Serialize, Deserialize, Clone, Debug, Hash, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverwriteTexture {
	#[rune(get, set)]
	pub specs: (ResourceSpecifier, Option<ResourceSpecifier>),

	#[serde(with = "serde_bytes")]
	#[rune(get, set)]
	pub data: Vec<u8>,

	#[rune(get, set, as_into = String)]
	pub data_extension: EcoString,

	#[rune(get, set)]
	pub metadata: (TextureType, RenderFormat, InterpretAs)
}

register_operation!(OverwriteTexture);

impl GraphOperation for OverwriteTexture {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		[Some(self.specs.0.to_owned()), self.specs.1.to_owned()]
			.into_iter()
			.flatten()
			.collect()
	}

	fn get_hash(&self) -> u64 {
		let mut hasher = Xxh3Default::new();
		self.hash(&mut hasher);
		hasher.digest()
	}

	#[try_fn]
	async fn evaluate(self, _: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		let texture_map = match self.data_extension.as_ref() {
			"dds" => TextureMapBuilder::from_dds(Cursor::new(&self.data)),
			"png" | "tga" => TextureMapBuilder::from_dynamic_image(
				image::load_from_memory_with_format(
					&self.data,
					match self.data_extension.as_ref() {
						"png" => image::ImageFormat::Png,
						"tga" => image::ImageFormat::Tga,
						_ => unreachable!()
					}
				)
				.wrap_err("Invalid image")?
			),
			_ => bail!("Unknown image format {}", self.data_extension)
		}
		.wrap_err("Failed to create TextureMapBuilder from image data")?
		.with_mipblock1(self.specs.1.is_some())
		.with_format(self.metadata.1.into())
		.texture_type(self.metadata.0.into())
		.interpret_as(self.metadata.2.into())
		.build(state.game.version.into())
		.wrap_err("Failed to build TextureMap from image data")?;

		let text_data = texture_map.pack_to_vec().wrap_err("Failed to pack TEXT data")?;

		let texd_data = if texture_map.has_mipblock1() {
			Some(
				texture_map
					.mipblock1()
					.ok_or_eyre("Failed to retrieve TEXD data from TextureMap")?
					.pack_to_vec(state.game.version.into())
					.wrap_err("Failed to pack TEXD data")?
			)
		} else {
			None
		};

		let text_mutation = Mutation::SetResourceValue {
			resource: self.specs.0.to_owned(),
			value: ResourceState {
				metadata: ResourceMetadata {
					id: self.specs.0.id,
					resource_type: resource_type!("TEXT"),
					compressed: ResourceMetadata::infer_compressed(resource_type!("TEXT")),
					scrambled: ResourceMetadata::infer_scrambled(resource_type!("TEXT")),
					references: if let Some(texd) = self.specs.1.as_ref() {
						vec![ResourceReference {
							resource: texd.id,
							flags: ReferenceFlags {
								reference_type: ReferenceType::Weak,
								..Default::default()
							}
						}]
					} else {
						vec![]
					}
				},
				data: text_data
			}
		};

		if let Some(texd) = self.specs.1
			&& let Some(texd_data) = texd_data
		{
			vec![
				text_mutation,
				Mutation::SetResourceValue {
					resource: texd.to_owned(),
					value: ResourceState {
						metadata: ResourceMetadata {
							id: texd.id,
							resource_type: resource_type!("TEXD"),
							compressed: ResourceMetadata::infer_compressed(resource_type!("TEXD")),
							scrambled: ResourceMetadata::infer_scrambled(resource_type!("TEXD")),
							references: vec![]
						},
						data: texd_data
					}
				},
			]
		} else {
			vec![text_mutation]
		}
	}
}
#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_TEXTURE: Analyser = Analyser {
	file_type: "texture.json",
	analyse: |world, game, mod_id, attribution, partition, file, file_contents| {
		let (data_path, data) = ["dds", "png", "tga"]
			.into_iter()
			.map(|ext| file.with_extension(ext))
			.find_map(|p| world.read_mod_file(mod_id, &p).ok().map(|x| (p, x)))
			.ok_or_eyre("Couldn't read any associated image file")?;

		let metadata: TextureMetadata = serde_json::from_slice(&file_contents)
			.wrap_err("Texture metadata JSON was not valid")
			.intentional()?;

		if let Some(text) = game.realise_resource_specifier(metadata.text, NominalPartition(partition.to_owned()))? {
			let texd = metadata
				.texd
				.map(|texd| game.realise_resource_specifier(texd, NominalPartition(partition.to_owned())))
				.transpose()?
				.flatten();

			Ok(vec![
				OverwriteTexture {
					specs: (text, texd),
					data,
					data_extension: data_path.extension().unwrap().into(),
					metadata: (metadata.texture_type, metadata.format, metadata.interpret_as)
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
pub struct OverwriteSFX {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub wem_id: u32,

	#[serde(with = "serde_bytes")]
	#[rune(get, set)]
	pub data: Vec<u8>
}

register_operation!(OverwriteSFX);

impl GraphOperation for OverwriteSFX {
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
		let sfx = state.get_resource(&self.spec).await?;

		log::trace!(target: attribution.source.as_str(), "Parsing WWEV");

		let mut evt =
			WwiseEvent::parse(state.game.version, &sfx.data, &sfx.metadata).wrap_err("Couldn't parse SFX data")?;

		evt.non_streamed
			.iter_mut()
			.find(|x| x.wem_id == self.wem_id)
			.ok_or_else(|| eyre!("No (non-streamed) audio object with the WEM ID {}", self.wem_id))
			.intentional()?
			.data = self.data;

		log::trace!(target: attribution.source.as_str(), "Serialising WWEV");

		let (data, metadata) = evt.generate(state.game.version);

		vec![Mutation::SetResourceValue {
			resource: self.spec,
			value: ResourceState { metadata, data }
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_SFX: Analyser = Analyser {
	file_type: "sfx.wem",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		if let Some(spec) = game.realise_resource_specifier(
			RuntimeID::from_str(
				file.file_name()
					.unwrap()
					.split('.')
					.next()
					.unwrap()
					.split('~')
					.next()
					.ok_or_eyre("sfx.wem filename must follow format RuntimeID~wemID")
					.intentional()?
			)?,
			NominalPartition(partition.to_owned())
		)? {
			Ok(vec![
				OverwriteSFX {
					spec,
					wem_id: file
						.file_name()
						.unwrap()
						.split('.')
						.next()
						.unwrap()
						.split('~')
						.nth(1)
						.ok_or_eyre("sfx.wem filename must follow format RuntimeID~wemID")
						.intentional()?
						.to_owned()
						.parse()?,
					data: file_contents
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
pub struct OverwriteSoundDefinitions {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub data: SoundDefinitions
}

register_operation!(OverwriteSoundDefinitions);

impl GraphOperation for OverwriteSoundDefinitions {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
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
	async fn evaluate(self, _: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		let (data, metadata) = self.data.generate(state.game.version)?;

		vec![Mutation::SetResourceValue {
			resource: self.spec,
			value: ResourceState { metadata, data }
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_SOUNDDEFS: Analyser = Analyser {
	file_type: "sounddefs.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: SoundDefinitions = serde_json::from_slice(&file_contents)
			.wrap_err("Sound definitions JSON was not valid")
			.intentional()?;

		if let Some(spec) = game.realise_resource_specifier(data.id, NominalPartition(partition.to_owned()))? {
			Ok(vec![OverwriteSoundDefinitions { spec, data }.into()])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct PatchSoundDefinitions {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	#[rune(get, set)]
	pub data: SoundDefinitions
}

register_operation!(PatchSoundDefinitions);

impl GraphOperation for PatchSoundDefinitions {
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
		let sdef = state.get_resource(&self.spec).await?;

		log::trace!(target: attribution.source.as_str(), "Parsing SDEF");

		let mut sdef = SoundDefinitions::parse(state.game.version, &sdef.data, &sdef.metadata)
			.wrap_err("Couldn't parse SDEF data")?;

		if let Some(name) = self.data.name {
			sdef.name = Some(name);
		}

		sdef.definitions.extend(self.data.definitions);

		log::trace!(target: attribution.source.as_str(), "Serialising SDEF");

		let (data, metadata) = sdef.generate(state.game.version)?;

		vec![Mutation::SetResourceValue {
			resource: self.spec,
			value: ResourceState { metadata, data }
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_SOUNDDEFS_PATCH: Analyser = Analyser {
	file_type: "sounddefs.patch.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: SoundDefinitions = serde_json::from_slice(&file_contents)
			.wrap_err("Sound definitions JSON was not valid")
			.intentional()?;

		if let Some(spec) = game.realise_resource_specifier(data.id, NominalPartition(partition.to_owned()))? {
			Ok(vec![PatchSoundDefinitions { spec, data }.into()])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct AddPackageDefinitionEntry {
	#[rune(get, set)]
	pub data: PackageDefinitionEntity
}

register_operation!(AddPackageDefinitionEntry);

impl GraphOperation for AddPackageDefinitionEntry {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
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
		vec![Mutation::AddPackageDefinitionEntry { data: self.data }]
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor_fn = Self::rune_construct, install_with = Self::rune_install)]
pub struct OverwriteBehaviorTree {
	#[rune(get, set)]
	pub spec: ResourceSpecifier,

	pub data: Value
}

impl OverwriteBehaviorTree {
	fn rune_construct(spec: ResourceSpecifier, data: rune::Value) -> Self {
		Self {
			spec,
			data: serde_json::to_value(data).unwrap_or(Value::Null)
		}
	}

	#[try_fn]
	fn rune_install(module: &mut rune::Module) -> Result<(), rune::ContextError> {
		module.field_function(&rune::runtime::Protocol::GET, "data", |s: &Self| -> rune::Value {
			serde_json::from_value(s.data.to_owned()).unwrap()
		})?;

		module.field_function(&rune::runtime::Protocol::SET, "data", |s: &mut Self, v: rune::Value| {
			s.data = serde_json::to_value(v).unwrap_or(Value::Null);
		})?;
	}
}

register_operation!(OverwriteBehaviorTree);

impl GraphOperation for OverwriteBehaviorTree {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
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
	async fn evaluate(self, _: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		vec![Mutation::SetResourceValue {
			value: ResourceState {
				metadata: ResourceMetadata {
					id: self.spec.id,
					resource_type: resource_type!("AIBZ"),
					compressed: ResourceMetadata::infer_compressed(resource_type!("AIBZ")),
					scrambled: ResourceMetadata::infer_scrambled(resource_type!("AIBZ")),
					references: vec![]
				},
				data: match state.game.version {
					GlacierGame::H1 => glacier_bin1::serialize(
						&serde_json::from_value::<hitman_behavior::h1::BehaviorTree>(self.data)
							.wrap_err("Couldn't parse behavior tree")
							.intentional()?
							.into_raw()
							.intentional()?
					)?,

					GlacierGame::H2 => glacier_bin1::serialize(
						&serde_json::from_value::<hitman_behavior::h2::BehaviorTree>(self.data)
							.wrap_err("Couldn't parse behavior tree")
							.intentional()?
							.into_raw()
							.intentional()?
					)?,

					GlacierGame::H3 => glacier_bin1::serialize(
						&serde_json::from_value::<hitman_behavior::h3::BehaviorTree>(self.data)
							.wrap_err("Couldn't parse behavior tree")
							.intentional()?
							.into_raw()
							.intentional()?
					)?,

					_ => bail!("Behavior trees are not supported in this game version")
				}
			},
			resource: self.spec
		}]
	}
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_BEHAVIOR_TREE: Analyser = Analyser {
	file_type: "behavior.txt",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data = String::from_utf8(file_contents)?;

		let (id, data) = data
			.split_once("---")
			.ok_or_eyre("Behavior tree must contain '---' separating ID and data")
			.intentional()?;

		let (id, data) = (id.trim(), data.trim());

		if game.version == GlacierGame::FL {
			log::debug!(target: attribution, "Skipping content file {} as behavior trees are not valid for current game", file.as_str());
			return Ok(vec![]);
		}

		if let Some(spec) =
			game.realise_resource_specifier(RuntimeID::from_str(id)?, NominalPartition(partition.to_owned()))?
		{
			Ok(vec![
				OverwriteBehaviorTree {
					spec,
					data: match game.version {
						GlacierGame::H1 => serde_json::to_value(
							&hitman_behavior::h1::BehaviorTree::from_pseudocode(data).intentional()?
						)?,

						GlacierGame::H2 => serde_json::to_value(
							&hitman_behavior::h2::BehaviorTree::from_pseudocode(data).intentional()?
						)?,

						GlacierGame::H3 => serde_json::to_value(
							&hitman_behavior::h3::BehaviorTree::from_pseudocode(data).intentional()?
						)?,

						_ => bail!("Behavior trees are not supported in this game version")
					}
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
pub struct OverwriteAspectEntity {
	#[rune(get, set)]
	pub factory: ResourceSpecifier,

	#[rune(get, set)]
	pub blueprint: ResourceSpecifier,

	#[rune(get, set)]
	pub resources: Vec<(RuntimeID, RuntimeID)>
}

register_operation!(OverwriteAspectEntity);

impl GraphOperation for OverwriteAspectEntity {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.factory.to_owned(), self.blueprint.to_owned()]
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
		vec![
			Mutation::SetResourceValue {
				value: ResourceState {
					metadata: ResourceMetadata {
						id: self.factory.id,
						resource_type: resource_type!("ASET"),
						compressed: ResourceMetadata::infer_compressed(resource_type!("ASET")),
						scrambled: ResourceMetadata::infer_scrambled(resource_type!("ASET")),
						references: self
							.resources
							.iter()
							.map(|(x, _)| ResourceReference {
								resource: *x,
								flags: Default::default()
							})
							.chain(std::iter::once(ResourceReference {
								resource: self.blueprint.id,
								flags: Default::default()
							}))
							.collect()
					},
					data: (0..=self.resources.len())
						.flat_map(|idx| (idx as u32).to_le_bytes())
						.collect()
				},
				resource: self.factory
			},
			Mutation::SetResourceValue {
				value: ResourceState {
					metadata: ResourceMetadata {
						id: self.blueprint.id,
						resource_type: resource_type!("ASEB"),
						compressed: ResourceMetadata::infer_compressed(resource_type!("ASEB")),
						scrambled: ResourceMetadata::infer_scrambled(resource_type!("ASEB")),
						references: self
							.resources
							.iter()
							.map(|(_, y)| ResourceReference {
								resource: *y,
								flags: Default::default()
							})
							.collect()
					},
					data: (0..self.resources.len())
						.flat_map(|idx| (idx as u32).to_le_bytes())
						.collect()
				},
				resource: self.blueprint
			},
		]
	}
}

#[derive(Serialize, Deserialize)]
pub struct AspectEntity {
	factory: RuntimeID,
	blueprint: RuntimeID,
	resources: Vec<AspectResource>
}

#[derive(Serialize, Deserialize)]
pub struct AspectResource {
	factory: RuntimeID,
	blueprint: RuntimeID
}

#[linkme::distributed_slice(ANALYSERS)]
static ANALYSE_ASPECT: Analyser = Analyser {
	file_type: "aspect.entity.json",
	analyse: |_, game, _, attribution, partition, file, file_contents| {
		let data: AspectEntity = serde_json::from_slice(&file_contents)
			.wrap_err("Aspect entity JSON was not valid")
			.intentional()?;

		if let Some(factory) = game.realise_resource_specifier(data.factory, NominalPartition(partition.to_owned()))?
			&& let Some(blueprint) =
				game.realise_resource_specifier(data.blueprint, NominalPartition(partition.to_owned()))?
		{
			Ok(vec![
				OverwriteAspectEntity {
					factory,
					blueprint,
					resources: data.resources.into_iter().map(|x| (x.factory, x.blueprint)).collect()
				}
				.into(),
			])
		} else {
			log::debug!(target: attribution, "Skipping content file {} as partition is not valid for current game", file.as_str());
			Ok(vec![])
		}
	}
};

#[derive(
	Serialize, Deserialize, Clone, Debug, better_rune_derive::Any, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[rune(item = ::simple_mod_framework::graph)]
#[rune(constructor)]
pub struct OverwriteRawResource {
	#[rune(get, set)]
	pub id: ResourceSpecifier,

	#[rune(get, set)]
	pub data: ResourceState
}

register_operation!(OverwriteRawResource);

impl GraphOperation for OverwriteRawResource {
	fn should_cache(&self) -> bool {
		false
	}

	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![]
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		vec![self.id.to_owned()]
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(&rkyv::to_bytes::<rancor::BoxedError>(self).expect("Couldn't serialise operation"))
	}

	#[try_fn]
	async fn evaluate(self, _: Attribution, _: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		vec![Mutation::SetResourceValue {
			resource: self.id,
			value: self.data
		}]
	}
}
