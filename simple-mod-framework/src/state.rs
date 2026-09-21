use std::{collections::VecDeque, fmt::Debug, fs, hash::Hash, ops::Deref, path::PathBuf, sync::Arc};

use color_eyre::{
	Section,
	eyre::{OptionExt, Result, WrapErr, bail, eyre}
};
use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_base::encryption::xtea::{Xtea, XteaConfig};
use glacier_commons::{
	game::GlacierGame,
	metadata::{ResourceMetadata, RuntimeID}
};
use identity_hash::BuildIdentityHasher;
use itertools::Itertools;
use lazy_regex::{regex_captures, regex_replace, regex_replace_all};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use rpkg_rs::resource::{
	package_builder::PackageResourceBuilder,
	pdefs::{
		PackageDefinitionParser, bond_parser::BondParser, h2016_parser::H2016Parser, hm2_parser::HM2Parser,
		hm3_parser::HM3Parser
	},
	resource_package::{ResourceReferenceFlags, ResourceReferenceFlagsStandard}
};
use serde::{Deserialize, Serialize};
use simple_mod_framework_core::{
	game::{GameContext, NominalPartition, RealPartition, ResourceSpecifier},
	intentional_halt,
	rkyv_helpers::AsSerde
};
use simple_mod_framework_types::{Config, HashMap, HashSet, ModID, PackageDefinitionEntity, PapayaSet, ServerSideData};
use tracing::instrument;
use tryvial::try_fn;
use uuid::Uuid;

use crate::{
	cache::Cache,
	deploy::{generate_rpkgs, port_resources},
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{Attribution, DeployGraph},
	scripts::JsonValue,
	world::{Mods, Output, Progress, World}
};

/// The state of a resource. Contains the metadata and the content of the resource.
#[derive(
	Hash,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
	Clone,
	better_rune_derive::Any,
	rkyv::Archive,
	rkyv::Serialize,
	rkyv::Deserialize,
)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE, PARTIAL_EQ, EQ)]
#[rune(constructor)]
pub struct ResourceState {
	#[rune(get, set)]
	pub metadata: ResourceMetadata,

	#[serde(with = "serde_bytes")]
	#[rune(get, set)]
	pub data: Vec<u8>
}

impl ResourceState {
	#[try_fn]
	pub fn into_builder(self, game: &GameContext) -> Result<PackageResourceBuilder> {
		let should_compress = self.metadata.compressed && self.data.len() > 1024;

		let system_memory_requirement =
			ResourceMetadata::calculate_system_memory_requirement(self.metadata.resource_type, &self.data)
				.unwrap_or(u32::MAX);

		let video_memory_requirement =
			ResourceMetadata::calculate_video_memory_requirement(self.metadata.resource_type, &self.data, game.version)
				.unwrap_or(u32::MAX);

		let mut resource = PackageResourceBuilder::from_memory(
			game.to_rrid(self.metadata.id),
			self.metadata.resource_type.as_ref(),
			self.data,
			should_compress.then_some(1),
			self.metadata.scrambled
		)
		.wrap_err("Couldn't construct resource builder")?;

		resource.with_memory_requirements(system_memory_requirement, video_memory_requirement);

		// TODO: Use legacy flags on legacy H1
		resource.with_references(self.metadata.references.into_iter().map(|reference| {
			(
				game.to_rrid(reference.resource),
				ResourceReferenceFlags::Standard(ResourceReferenceFlagsStandard::from_bits(
					reference.flags.as_modern()
				))
			)
		}));

		resource
	}
}

impl Debug for ResourceState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "resource {} ({} bytes)", self.metadata.id, self.data.len())
	}
}

pub type ResourceStates = VecDeque<(Option<Attribution>, ResourceState)>;

/// The state of a specific deployment, including the working values of all resources being modified by the deployment.
pub struct DeployContext {
	/// All resource IDs (and their partitions) that will be required or affected by any graph node (i.e. all resources known to certainly exist by the end of deployment).
	/// Sorted by the order of the partition in the game files.
	pub all_relevant_resources: HashMap<RuntimeID, Vec<RealPartition>, BuildIdentityHasher<u64>>,

	/// The working values of all resources.
	pub resource_states: HashMap<ResourceSpecifier, Arc<tokio::sync::Mutex<ResourceStates>>>,

	/// The current state of the packagedefinition.txt file.
	pub package_definition: Mutex<String>,

	/// The current state of the thumbs.dat file.
	pub thumbs: Mutex<String>,

	pub server_side_data: Mutex<ServerSideData>,
	pub server_side_assets: Mutex<HashMap<Uuid, Vec<u8>>>,
	pub world_map_metadata: Mutex<HashSet<RuntimeID, BuildIdentityHasher<u64>>>,

	/// Resource to port, partition to port for
	pub resources_to_port: PapayaSet<(RuntimeID, RealPartition)>
}

impl DeployContext {
	#[try_fn]
	pub fn new(config: &Config, game: &GameContext) -> Result<Self> {
		Self {
			all_relevant_resources: HashMap::default(),
			resource_states: HashMap::default(),
			package_definition: game.clean_package_definition.to_owned().into(),
			thumbs: {
				let data = fs::read(game.retail_path.join("thumbs.dat")).wrap_err("Couldn't read thumbs.dat")?;

				let xtea = Xtea::new(if game.version == GlacierGame::FL {
					XteaConfig::KNT
				} else {
					XteaConfig::Woa
				});

				let data = if xtea.is_encrypted_text_file(&data) {
					xtea.decrypt_text_file(&data)
						.map_err(|x| eyre!("XTEA decryption error for thumbs: {:?}", x))?
				} else {
					String::from_utf8(data)?
				};

				if let Some(boot_scene) = &config.boot_scene {
					regex_replace_all!("SCENE_FILE=.*?\r?\n", &data, &format!("SCENE_FILE={boot_scene}\n"))
						.into_owned()
						.into()
				} else if config.skip_intro && game.version != GlacierGame::FL {
					regex_replace_all!(
						"SCENE_FILE=.*?\r?\n",
						&data,
						"SCENE_FILE=assembly:/_PRO/Scenes/Frontend/MainMenu.entity\n"
					)
					.into_owned()
					.into()
				} else {
					regex_replace_all!(
						"SCENE_FILE=.*?\r?\n",
						&data,
						if game.version == GlacierGame::FL {
							"SCENE_FILE=assembly:/_knt/scenes/game.entity\n"
						} else {
							"SCENE_FILE=assembly:/_PRO/Scenes/Frontend/Boot.entity\n"
						}
					)
					.into_owned()
					.into()
				}
			},
			server_side_data: Default::default(),
			server_side_assets: Default::default(),
			world_map_metadata: Default::default(),
			resources_to_port: Default::default()
		}
	}
}

impl Clone for DeployContext {
	fn clone(&self) -> Self {
		Self {
			all_relevant_resources: self.all_relevant_resources.clone(),
			resource_states: self.resource_states.clone(),
			package_definition: self.package_definition.lock().clone().into(),
			thumbs: self.thumbs.lock().clone().into(),
			server_side_data: self.server_side_data.lock().clone().into(),
			server_side_assets: self.server_side_assets.lock().clone().into(),
			world_map_metadata: self.world_map_metadata.lock().clone().into(),
			resources_to_port: self.resources_to_port.clone()
		}
	}
}

/// The global state of the current deployment.
pub struct State<W: World + ?Sized> {
	pub config: Arc<Config>,

	/// The TonyTools hash list.
	pub localisation_hash_list: Arc<tonytools::hashlist::HashList>,

	/// The game currently being deployed to.
	pub game: Arc<GameContext>,

	pub world: Arc<W>,

	pub deployment: DeployContext
}

impl<W: World + Mods + Output + Progress> State<W> {
	#[try_fn]
	#[instrument(skip_all)]
	#[wrap_err("Couldn't prepare state")]
	pub async fn prepare(
		&mut self,
		graph: &DeployGraph,
		peacock_plugins: Vec<PathBuf>,
		sdk_mods: Vec<(ModID, PathBuf)>,
		resources_to_port: HashSet<(RuntimeID, RealPartition)>,
		cache: &Cache
	) -> Result<()> {
		self.deployment
			.server_side_data
			.lock()
			.peacock_plugins
			.extend(peacock_plugins);

		for (id, dll) in sdk_mods {
			self.world.emit_sdk_mod(
				format!(
					"{} - {}.dll",
					regex_replace_all!(
						"[^a-zA-Z0-9 ]",
						self.world
							.get_mod_manifest(&id)?
							.name
							.first_specified()
							.expect("No localisation specified"),
						""
					),
					dll.file_stem()
						.ok_or_eyre("Couldn't get SDK mod filename")?
						.to_string_lossy()
				),
				dll
			)?;
		}

		for res in resources_to_port {
			self.deployment.resources_to_port.pin().insert(res);
		}

		log::info!("Preloading resources");
		{
			let _span = tracing::info_span!("Preloading resources").entered();

			for resource in graph
				.nodes()
				.values()
				.flat_map(|node| node.operation.resource_deps((&*self.game).into()))
				.sorted_by_cached_key(|resource| {
					self.game
						.game_files
						.partitions
						.iter()
						.position(|x| x.partition_info().name.as_deref().unwrap() == resource.partition.as_str())
						.unwrap()
				}) {
				self.deployment
					.all_relevant_resources
					.entry(resource.id)
					.or_default()
					.push(resource.partition.to_owned());

				self.deployment
					.resource_states
					.insert(resource, tokio::sync::Mutex::new(Default::default()).into());
			}

			let preloads = graph
				.nodes()
				.iter()
				.flat_map(|(id, node)| {
					if !node.operation.should_cache() || !cache.is_node_cached(id) {
						node.operation
							.required((&*self.game).into())
							.into_iter()
							.map(|x| (node.attribution.source.as_str(), x))
							.collect()
					} else {
						vec![]
					}
				})
				.collect_vec();

			if !preloads.is_empty() {
				let progress = self
					.world
					.start_progress("Preloading resources", preloads.len() as u64)?;

				preloads.into_par_iter().try_for_each(|(attribution, spec)| {
					self.preload(attribution, spec)?;
					self.world.advance_progress(progress, 1)?;
					color_eyre::eyre::Ok(())
				})?;

				self.world.finish_progress(progress)?;
			}
		}
	}
}

impl<W: World> State<W> {
	/// Stage a resource. This will remove all older copies of the resource from the global state. Only to be used at an end node in the graph.
	#[try_fn]
	#[wrap_err("Couldn't stage resource {:?}", spec)]
	#[instrument]
	pub async fn stage(&self, spec: ResourceSpecifier) -> Result<()> {
		let mut states = self
			.deployment
			.resource_states
			.get(&spec)
			.ok_or_else(|| eyre!("No such resource: {:?}", spec))?
			.lock()
			.await;

		let mut res = states.pop_back().unwrap().1;
		res.metadata.id = spec.id;
		*states = [(None, res)].into_iter().collect();
	}

	/// Ready a resource for use by extracting it and inserting it into resource_states.
	#[try_fn]
	#[wrap_err("Couldn't preload resource {:?}", spec)]
	#[instrument]
	pub fn preload(&self, attribution: &str, spec: ResourceSpecifier) -> Result<()> {
		// This mutex shouldn't be contended so easier to do this in Rayon than Tokio
		let mut states = futures::executor::block_on(
			self.deployment
				.resource_states
				.get(&spec)
				.ok_or_eyre("No such known resource")?
				.lock()
		);

		if !states.is_empty() {
			return Ok(());
		}

		let Some(partition) = self
			.game
			.game_files
			.partitions
			.iter()
			.find(|x| x.partition_info().name.as_deref().unwrap() == spec.partition.as_str())
		else {
			log::debug!(target: attribution, "Partition {} not found, skipping", spec.partition.0);
			return Ok(());
		};

		let Ok(res_info) = partition.get_resource_info(&self.game.to_rrid(spec.id)) else {
			log::debug!(target: attribution, "Resource {spec:?} not found in partition, skipping");
			return Ok(());
		};

		let res_data = partition
			.read_resource(&self.game.to_rrid(spec.id))
			.context("Couldn't extract resource using rpkg-rs")?;

		states.push_back((
			None,
			ResourceState {
				metadata: res_info.try_into().wrap_err("Resource info was not valid")?,
				data: res_data
			}
		));
	}

	#[try_fn]
	pub async fn get_resource(
		&self,
		required: &ResourceSpecifier
	) -> Result<tokio::sync::MappedMutexGuard<'_, ResourceState>> {
		tokio::sync::MutexGuard::map(
			self.get_resource_states(required).await?,
			|states: &mut ResourceStates| &mut states.back_mut().unwrap().1
		)
	}

	pub async fn get_resource_states(
		&self,
		required: &ResourceSpecifier
	) -> Result<tokio::sync::MutexGuard<'_, ResourceStates>> {
		let states = self.deployment.resource_states.get(required).unwrap().lock().await;
		let mut res = (!states.is_empty())
			.then_some(states)
			.ok_or_else(|| eyre!("Necessary resource {:?} not preloaded", required))
			.suggestion("is a file being patched that has not yet been deployed?");

		if self.game.version != GlacierGame::H3
			&& required.partition
				!= *NominalPartition("super".into())
					.real_candidates(&*self.game)
					.unwrap()
					.first()
					.unwrap()
		{
			res = res.suggestion("is the game DLC for this location installed?");
		}

		res
	}

	/// Apply a mutation to the state.
	#[try_fn]
	#[wrap_err("Couldn't apply mutation to state")]
	#[instrument]
	pub async fn apply(&self, attribution: &Attribution, mutation: Mutation, warn_on_identical: bool) -> Result<()> {
		match mutation {
			Mutation::SetResourceValue { resource, value } => {
				let prev_refs = if let Some(states) = self.deployment.resource_states.get(&resource)
					&& let mut states = states.lock().await
					&& let Some((_, last_state)) = states.back()
				{
					if *last_state == value {
						// No actual mutation to apply
						if warn_on_identical {
							self.world.emit_diagnostic(Diagnostic {
								kind: DiagnosticKind::ResourceAlreadyIdentical {
									resource: resource.clone()
								},
								target: attribution.to_diagnostic_target()
							})?;
						}

						if value.metadata.resource_type != "TEMP" && value.metadata.resource_type != "TBLU" {
							// Not much use for full history
							states.retain_back(5);
						}

						states.push_back((Some(attribution.to_owned()), value));

						return Ok(());
					}

					last_state
						.metadata
						.references
						.iter()
						.map(|reference| reference.resource)
						.collect()
				} else if let Some((size, metadata)) = self.game.get_metadata(&resource) {
					if warn_on_identical
						&& size == (value.data.len() as u32)
						&& metadata == value.metadata
						&& self.game.get_data(&resource)? == value.data
					{
						// No actual mutation to apply
						self.world.emit_diagnostic(Diagnostic {
							kind: DiagnosticKind::ResourceAlreadyIdentical {
								resource: resource.clone()
							},
							target: attribution.to_diagnostic_target()
						})?;

						let mut states = self
							.deployment
							.resource_states
							.get(&resource)
							.expect("No such resource is known")
							.lock()
							.await;

						if value.metadata.resource_type != "TEMP" && value.metadata.resource_type != "TBLU" {
							// Not much use for full history
							states.retain_back(5);
						}

						states.push_back((Some(attribution.to_owned()), value));

						return Ok(());
					}

					metadata.references.iter().map(|reference| reference.resource).collect()
				} else {
					HashSet::new()
				};

				// Automatically port any necessary dependencies
				let accessible_partitions = self.game.get_accessible_partitions(resource.partition.as_str())?;

				let resources_to_port = self.deployment.resources_to_port.pin_owned();
				value
					.metadata
					.references
					.par_iter()
					.filter(|reference| !prev_refs.contains(&reference.resource))
					.for_each(|reference| {
						let id = reference.resource;

						if let Some(partitions) = self.game.vanilla_resources.get(&id)
							&& !partitions.is_empty()
						{
							if !partitions.iter().any(|x| accessible_partitions.contains(x)) {
								if let Some(partitions) = self.deployment.all_relevant_resources.get(&id)
									&& partitions.iter().any(|x| accessible_partitions.contains(x))
								{
									return;
								}

								resources_to_port.insert((id, resource.partition.to_owned()));
							}
						} else {
							// Resource is added by a mod, on deletion lists or doesn't exist
							if let Some(partitions) = self.deployment.all_relevant_resources.get(&id)
								&& !partitions.is_empty()
							{
								if !partitions.iter().any(|x| accessible_partitions.contains(x)) {
									// If it's added by a mod but not in an accessible partition, port it
									resources_to_port.insert((id, resource.partition.to_owned()));
								}
							} else if self.game.deleted_resources.get(&id).is_some_and(|x| !x.is_empty()) {
								// Deleted resources are not implicitly ported
								if !resources_to_port.contains(&(id, resource.partition.to_owned()))
									&& !resources_to_port
										.iter()
										.any(|(res, partition)| *res == id && accessible_partitions.contains(partition))
								{
									self.world
										.emit_diagnostic(Diagnostic {
											kind: DiagnosticKind::ReferencedResourceDeleted {
												resource_id: reference.resource,
												reference_type: reference.flags.reference_type
											},
											target: attribution.to_diagnostic_target()
										})
										.unwrap();
								}
							} else {
								// If it doesn't exist, we can't port it
								self.world
									.emit_diagnostic(Diagnostic {
										kind: DiagnosticKind::ReferencedResourceNonexistent {
											resource_id: reference.resource,
											reference_type: reference.flags.reference_type
										},
										target: attribution.to_diagnostic_target()
									})
									.unwrap();
							}
						}
					});

				let mut states = self
					.deployment
					.resource_states
					.get(&resource)
					.expect("No such resource is known")
					.lock()
					.await;

				if value.metadata.resource_type != "TEMP" && value.metadata.resource_type != "TBLU" {
					// Not much use for full history
					states.retain_back(5);
				}

				states.push_back((Some(attribution.to_owned()), value));
			}

			Mutation::AddServerSideContract { contract, data } => {
				let asset_id = Uuid::new_v4();
				self.deployment
					.server_side_assets
					.lock()
					.insert(asset_id, serde_json::to_vec(&data)?);
				self.deployment
					.server_side_data
					.lock()
					.contracts
					.insert(contract, asset_id);
			}

			Mutation::AddPackageDefinitionEntry { data } => {
				let mut package_definition = self.deployment.package_definition.lock();

				let partition = NominalPartition(data.partition.deref().into());

				if let Some((_, suffix)) = regex_captures!(r"\.pc_([a-z_]+)$", &data.path) {
					self.world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::PackageDefinitionPlatformPath {
							suffix: suffix.to_string()
						},
						target: attribution.to_diagnostic_target()
					})?;
				}

				let id = if self.game.version == GlacierGame::FL {
					RuntimeID::from_path(&data.path)
				} else {
					RuntimeID::from_path(&regex_replace!(r"\.(?:pc_)?([a-z_]+)$", &data.path, ".pc_$1"))
				};

				if let Some(candidates) = partition.real_candidates(&*self.game) {
					// Use the partition that the resource is actually in (or will be by the end of deploy)
					// This therefore is correct for both existing vanilla scenes being added and new ones
					let real_partition = candidates
						.iter()
						.find(|x| {
							self.deployment
								.all_relevant_resources
								.get(&id)
								.is_some_and(|y| y.contains(*x))
								|| self.game.vanilla_resources.get(&id).is_some_and(|y| y.contains(*x))
						})
						.unwrap_or(
							candidates
								.first()
								.ok_or_eyre("No real candidates for nominal partition")?
						);

					let real_partition = &real_partition.0;

					let partitions = match self.game.version {
						GlacierGame::H1 => H2016Parser::parse(package_definition.as_bytes()),
						GlacierGame::H2 => HM2Parser::parse(package_definition.as_bytes()),
						GlacierGame::H3 => HM3Parser::parse(package_definition.as_bytes()),
						GlacierGame::FL => BondParser::parse(package_definition.as_bytes())
					}
					.wrap_err("Couldn't read packagedefinition")?;

					if let Some(p) = partitions.iter().find(|x| x.name.as_deref().unwrap() == real_partition)
						&& p.roots.iter().any(|x| x.resource_path() == *data.path)
					{
						// Already in packagedefinition
						return Ok(());
					}

					let new = match self.game.version {
						GlacierGame::H1 => Regex::new(&format!(
							r"## --- (Chunk |DLC)(\d+) +{}\r?\n#(chunk|dlc) patchlevel=(\d+)\r?\n",
							regex::escape(real_partition.as_str())
						))?
						.replace(
							&package_definition,
							format!(
								"## --- $1$2 {}\r\n#$3 patchlevel=$4\r\n{}\r\n",
								real_partition, data.path
							)
						),

						GlacierGame::H2 => Regex::new(&format!(
							r"// --- (Chunk|DLC) (\d+) +{}\r?\n@(chunk|dlc) patchlevel=(\d+)\r?\n",
							regex::escape(real_partition.as_str())
						))?
						.replace(
							&package_definition,
							format!(
								"// --- $1 $2 {}\r\n@$3 patchlevel=$4\r\n{}\r\n",
								real_partition, data.path
							)
						),

						GlacierGame::H3 | GlacierGame::FL => Regex::new(&format!(
							r"@partition name={} parent=(.*?) type=(.*?) patchlevel=(\d+)\r?\n",
							regex::escape(real_partition.as_str())
						))?
						.replace(
							&package_definition,
							format!(
								"@partition name={} parent=$1 type=$2 patchlevel=$3\r\n{}\r\n",
								real_partition, data.path
							)
						)
					};

					if *package_definition == new {
						bail!(
							"Couldn't find packagedefinition partition {} in which to add {}",
							real_partition,
							data.path
						);
					} else {
						*package_definition = new.into_owned();
					}
				} else {
					log::debug!(target: attribution.source.as_str(), "Skipping packagedefinition entry for {} because partition {} is not valid for current game", data.path, *partition);
				}
			}

			Mutation::RegisterWorldMapMetadata { id } => {
				self.deployment.world_map_metadata.lock().insert(id);
			}
		};
	}

	/// Patch thumbs/packagedefinition as necessary. Only to be used at the end of deployment, after the graph has been evaluated.
	#[try_fn]
	pub fn patch_configs(&mut self, ported_to_partitions: &[&str]) -> Result<()> {
		log::info!("Patching game");

		{
			let mut thumbs = self.deployment.thumbs.lock();

			if self.deployment.server_side_data.lock().dynamic_resources_disabled {
				log::debug!("Dynamic resources have been disabled to allow repository mods to work online.");

				if !thumbs.contains("ConsoleCmd OnlineResources_Disable 1") {
					*thumbs = thumbs.replace("[Hitman5]\n", "[Hitman5]\nConsoleCmd OnlineResources_Disable 1\n");
				}
			} else {
				*thumbs = thumbs.replace("[Hitman5]\nConsoleCmd OnlineResources_Disable 1\n", "[Hitman5]\n");
			}
		}

		{
			let mut package_definition = self.deployment.package_definition.lock();

			let partitions = match self.game.version {
				GlacierGame::H1 => H2016Parser::parse(package_definition.as_bytes()),
				GlacierGame::H2 => HM2Parser::parse(package_definition.as_bytes()),
				GlacierGame::H3 => HM3Parser::parse(package_definition.as_bytes()),
				GlacierGame::FL => BondParser::parse(package_definition.as_bytes())
			}
			.wrap_err("Couldn't read packagedefinition")?;

			*package_definition = if self.game.version == GlacierGame::H1 {
				let patched_partitions = self
					.deployment
					.resource_states
					.keys()
					.map(|resource| resource.partition.as_str())
					.chain(ported_to_partitions.iter().copied())
					.unique()
					.map(|partition| {
						partitions
							.iter()
							.position(|x| x.name.as_deref().is_some_and(|y| y == partition))
							.ok_or_eyre("No such partition")
					})
					.collect::<Result<Vec<_>>>()?;

				let mut idx = 0;
				regex_replace_all!(
					r"patchlevel=([0-9]+)",
					&format!("## Patched by Simple Mod Framework.\r\n{package_definition}"),
					|m: &str, p: &str| {
						idx += 1;
						if patched_partitions.contains(&(idx - 1)) {
							format!("patchlevel={}", p.parse::<usize>().unwrap() + 1)
						} else {
							m.to_owned()
						}
					}
				)
				.to_string()
			} else {
				regex_replace_all!(r"patchlevel=([0-9]+)", &package_definition, "patchlevel=100").to_string()
			};
		}
	}
}

impl<W: World + Progress + Output> State<W> {
	/// Check for resource leaks, port resources, finalise the state and emit output files.
	#[try_fn]
	#[instrument(skip_all)]
	pub async fn finish(
		mut self
	) -> Result<(
		Arc<Config>,
		Arc<GameContext>,
		Arc<W>,
		ServerSideData,
		HashMap<Uuid, Vec<u8>>
	)> {
		let leaks = self
			.deployment
			.resource_states
			.iter_mut()
			.filter_map(|(x, y)| (Arc::get_mut(y).unwrap().get_mut().len() != 1).then_some(format!("{x:?}")))
			.collect_vec();

		if !leaks.is_empty() {
			intentional_halt!(format!(
				"Resource leak detected! {} resource{} been leaked: {}",
				leaks.len(),
				if leaks.len() > 1 { "s have" } else { " has" },
				leaks.join(", ")
			));
		}

		log::info!("Porting resources");
		let port_builders = port_resources(&self.world, &self.game, &self.deployment)?;

		self.patch_configs(&port_builders.keys().map(|x| x.as_str()).collect_vec())?;

		self.world.emit_thumbs(self.deployment.thumbs.into_inner())?;

		let package_definition = self.deployment.package_definition.into_inner();

		let partitions = match self.game.version {
			GlacierGame::H1 => H2016Parser::parse(package_definition.as_bytes()),
			GlacierGame::H2 => HM2Parser::parse(package_definition.as_bytes()),
			GlacierGame::H3 => HM3Parser::parse(package_definition.as_bytes()),
			GlacierGame::FL => BondParser::parse(package_definition.as_bytes())
		}
		.wrap_err("Couldn't read packagedefinition")?;

		self.world.emit_package_definition(package_definition)?;

		log::info!("Generating RPKGs");

		let mut staged: HashMap<EcoString, Vec<ResourceState>> = HashMap::default();

		for (partition, resource) in self.deployment.resource_states.into_iter().map(|(resource, states)| {
			(
				resource.partition.0,
				Arc::into_inner(states).unwrap().into_inner().pop_front().unwrap().1
			)
		}) {
			staged.entry(partition).or_default().push(resource);
		}

		generate_rpkgs(&self.world, &self.game, partitions, staged, port_builders)?;

		(
			self.config,
			self.game,
			self.world,
			self.deployment.server_side_data.into_inner(),
			self.deployment.server_side_assets.into_inner()
		)
	}
}

impl<W: World> Debug for State<W> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("State (opaque)")
	}
}

#[derive(
	Serialize, Deserialize, Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, better_rune_derive::Any,
)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE)]
pub enum Mutation {
	#[rune(constructor)]
	SetResourceValue {
		#[rune(get, set)]
		resource: ResourceSpecifier,

		#[rune(get, set)]
		value: ResourceState
	},

	#[rune(constructor)]
	AddServerSideContract {
		#[rune(get, set)]
		contract: String,

		#[rkyv(with = AsSerde)]
		#[rune(get, set)]
		data: JsonValue
	},

	#[rune(constructor)]
	AddPackageDefinitionEntry {
		#[rune(get, set)]
		data: PackageDefinitionEntity
	},

	#[rune(constructor)]
	RegisterWorldMapMetadata {
		#[rune(get, set)]
		id: RuntimeID
	}
}
