use std::{
	fs,
	path::{Path, PathBuf},
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering}
	}
};

use color_eyre::{
	Section, SectionExt,
	eyre::{OptionExt, Result, WrapErr, bail, eyre}
};
use delegate::delegate;
use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_commons::{
	game::GlacierGame,
	metadata::{ResourceMetadata, RuntimeID}
};
use glacier_formats::sdef::SoundDefinitions;
use identity_hash::BuildIdentityHasher;
use itertools::Itertools;
use papaya::Guard;
use pico_args::Arguments;
use quickentity_rs::{entity::Entity, generate_patch};
use rayon::prelude::*;
use relative_path::{RelativePath, RelativePathBuf};
use rpkg_rs::resource::{
	package_builder::{PackageBuilder, PackageResourceBuilder},
	pdefs::{PartitionId, PartitionInfo},
	resource_package::PackageVersion,
	resource_partition::PatchId,
	runtime_resource_id::RuntimeResourceID
};
use simple_mod_framework_core::{
	game::{Filesystem, GameContext, GameOutput, NominalPartition, RealPartition, ResourceSpecifier},
	utils::ResultExt
};
use simple_mod_framework_types::{
	Config, HashMap, HashSet, Manifest, ModID, ModOptionData, NonEmptyString, PapayaSet, VersionPlatform
};
use tracing::instrument;
use tryvial::try_fn;
use velcro::vec;

use crate::{
	analysis::{analyse, get_effective_option_values, merge_option_data},
	cache::Cache,
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{self, DeployGraph, GraphNode, NodeOperation, Operation},
	state::{DeployContext, ResourceState, ResourceStates, State},
	topo_sort::topological_sort,
	world::{Diagnostics, LogDiagnostics, Mods, ModsWritable, Output, Progress, World}
};

/// Sort the deploy order (stably) according to manifest directives. Returns whether the deploy order was modified.
#[try_fn]
#[instrument(skip_all)]
pub fn sort_deploy_order(config: &mut Config, world: &impl Mods, platform: VersionPlatform) -> Result<bool> {
	let mut topo_dependencies = HashMap::<ModID, HashSet<ModID>>::default();

	for mod_id in &config.deploy_order {
		let mut manifest = (*world.get_mod_manifest(mod_id)?).to_owned();

		let default_mod_options = HashMap::default();
		let selected_mod_options = config.mod_options.get(mod_id).unwrap_or(&default_mod_options);

		let enabled_mod_versions = config
			.deploy_order
			.iter()
			.map(|mod_id| Ok((mod_id.to_owned(), world.get_mod_manifest(mod_id)?.version.0.to_owned())))
			.collect::<Result<HashMap<_, _>>>()?
			.into();

		let mut mod_options = HashMap::default();
		for option in &manifest.options {
			get_effective_option_values(
				world,
				platform,
				config,
				&enabled_mod_versions,
				selected_mod_options,
				&mut mod_options,
				option
			)?;
		}

		for option in manifest.options {
			merge_option_data(
				world,
				platform,
				config,
				&enabled_mod_versions,
				&mod_options,
				&mut manifest.data,
				option
			)?;
		}

		for deploy_after in &manifest.data.deploy_after {
			if config.deploy_order.contains(&deploy_after.id)
				&& deploy_after
					.version
					.matches(&world.get_mod_manifest(&deploy_after.id)?.version)
			{
				// This mod should deploy after the other mod, which means it "depends" on it
				topo_dependencies
					.entry(mod_id.to_owned())
					.or_default()
					.insert(deploy_after.id.to_owned());
			}
		}

		for deploy_before in &manifest.data.deploy_before {
			if config.deploy_order.contains(&deploy_before.id)
				&& deploy_before
					.version
					.matches(&world.get_mod_manifest(&deploy_before.id)?.version)
			{
				// This mod should deploy before the other mod, which means the other mod "depends" on this one
				topo_dependencies
					.entry(deploy_before.id.to_owned())
					.or_default()
					.insert(mod_id.to_owned());
			}
		}
	}

	let new = topological_sort(
		config
			.deploy_order
			.iter()
			.map(|id| {
				(
					id.to_owned(),
					topo_dependencies
						.get(id)
						.map(|x| x.to_owned())
						.unwrap_or_default()
						.into_iter()
						.collect()
				)
			})
			.collect_vec()
	)
	.wrap_err("Couldn't sort mods")
	.intentional()
	.note(
		"this happens when some mod X needs to deploy both before and after another mod Y, because of \
		 deployBefore/deployAfter manifest entries"
	)
	.suggestion("remove any recently added mods and try again")
	.with_section(|| {
		config
			.deploy_order
			.iter()
			.filter_map(|mod_id| {
				let manifest = world.get_mod_manifest(mod_id).ok()?;
				if manifest.data.deploy_before.is_empty() && manifest.data.deploy_after.is_empty() {
					None
				} else {
					Some(format!(
						"{}\n- must deploy before: {}\n- must deploy after: {}",
						manifest.id,
						manifest
							.data
							.deploy_before
							.iter()
							.map(|x| x.to_string())
							.collect_vec()
							.join(", "),
						manifest
							.data
							.deploy_after
							.iter()
							.map(|x| x.to_string())
							.collect_vec()
							.join(", ")
					))
				}
			})
			.collect_vec()
			.join("\n\n")
			.header("Manifest entries:")
	})
	.with_section(|| {
		topo_dependencies
			.iter()
			.map(|(m, deps)| {
				format!(
					"{}: must deploy after {}",
					m,
					deps.iter().map(|x| x.to_string()).collect_vec().join(", ")
				)
			})
			.collect_vec()
			.join("\n")
			.header("Final calculation:")
	})?;

	let changed = config.deploy_order != new;
	config.deploy_order = new;
	changed
}

/// Apply all possible automatic migrations to the graph and persist them (by overwriting the mod files on disk).
/// Will automatically re-analyse the graph as many times as necessary.
#[try_fn]
#[instrument(skip_all)]
#[wrap_err("Couldn't apply automatic migrations")]
pub fn migrate(
	config: &Arc<Config>,
	world: &Arc<impl Mods + ModsWritable + Diagnostics + Send + Sync + 'static>,
	game: &Arc<GameContext>,
	cache: &Cache,
	deploy_graph: &mut DeployGraph,
	peacock_plugins: &mut Vec<PathBuf>,
	sdk_mods: &mut Vec<(ModID, PathBuf)>,
	resources_to_port: &mut HashSet<(RuntimeID, RealPartition)>
) -> Result<()> {
	loop {
		let mut need_reanalysis = false;

		for node in deploy_graph
			.nodes()
			.values()
			.filter(|node| node.attribution.script_identifier.is_none())
		{
			if let NodeOperation::Uncached(operation) = &node.operation {
				// TEMP/TBLU -> entity.patch.json
				if let Operation::OverwriteRawResource(graph::OverwriteRawResource { id, data }) = &**operation
					&& data.metadata.resource_type == "TEMP"
					&& game.vanilla_resources.contains_key(&id.id)
				{
					macro_rules! impl_game {
						($game:ident, $factory:ident) => {{
							let factory =
								glacier_bin1::deserialize::<glacier_bin1::game::$game::$factory>(&data.data)
									.wrap_err("Couldn't convert TEMP file")
									.intentional()?;

							let file_path = &node.attribution.source;
							let metadata_path = file_path.with_extension("TEMP.metadata.json");

							let tblu_hash = data
								.metadata
								.references
								.get(usize::try_from(factory.blueprint_index_in_resource_header)?)
								.ok_or_eyre("Blueprint index invalid")
								.intentional()?
								.resource;

							if let Some(tblu_node) = deploy_graph
								.nodes()
								.values()
								.filter(|tblu_node| tblu_node.attribution.script_identifier.is_none())
								.find(|tblu_node| {
									node.attribution.mod_id == tblu_node.attribution.mod_id
										&& matches!(
											&tblu_node.operation,
											NodeOperation::Uncached(op)
												if matches!(
													&**op,
													Operation::OverwriteRawResource(graph::OverwriteRawResource { id, .. })
														if id.id == tblu_hash
												)
										)
								}) {
								let tblu_path = &tblu_node.attribution.source;
								let tblu_metadata_path = tblu_path.with_extension("TBLU.metadata.json");
								if world.read_mod_file(&node.attribution.mod_id, tblu_path).is_ok()
									&& let NodeOperation::Uncached(op) = &tblu_node.operation
									&& let Operation::OverwriteRawResource(graph::OverwriteRawResource {
										data: tblu_data, ..
									}) = &**op
								{
									// Convert to patch
									world.emit_diagnostic(Diagnostic {
										kind: DiagnosticKind::ApplyingMigration {
											message: format!(
												"This mod overwrites a base game entity ({id:?}) with a raw resource. This \
												will be converted into a patch."
											)
										},
										target: node.attribution.to_diagnostic_target()
									})?;

									let blueprint =
										glacier_bin1::deserialize::<glacier_bin1::game::$game::STemplateEntityBlueprint>(
											&tblu_data.data
										)
										.wrap_err("Couldn't convert TBLU file")
										.intentional()?;

									let entity = Entity::from_game(
										&factory,
										&data.metadata.to_owned(),
										&blueprint,
										&tblu_data.metadata.to_owned(),
										true
									)
									.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

									let vanilla_fac_spec = game.infer_resource_specifier(data.metadata.id)?.unwrap();
									let vanilla_blu_spec = game.infer_resource_specifier(tblu_data.metadata.id)?.unwrap();

									let (vanilla_fac_meta, vanilla_fac_data) = {
										let partition = game
											.game_files
											.partitions
											.iter()
											.find(|p| {
												p.partition_info().name.as_deref() == Some(vanilla_fac_spec.partition.as_str())
											})
											.unwrap();

										(
											ResourceMetadata::try_from(
												partition.get_resource_info(&game.to_rrid(vanilla_fac_spec.id))?
											)?,
											partition.read_resource(&game.to_rrid(vanilla_fac_spec.id))?
										)
									};

									let (vanilla_blu_meta, vanilla_blu_data) = {
										let partition = game
											.game_files
											.partitions
											.iter()
											.find(|p| {
												p.partition_info().name.as_deref() == Some(vanilla_blu_spec.partition.as_str())
											})
											.unwrap();

										(
											ResourceMetadata::try_from(
												partition.get_resource_info(&game.to_rrid(vanilla_blu_spec.id))?
											)?,
											partition.read_resource(&game.to_rrid(vanilla_blu_spec.id))?
										)
									};

									let vanilla_fac = glacier_bin1::deserialize::<
										glacier_bin1::game::$game::$factory
									>(&vanilla_fac_data)?;

									let vanilla_blu = glacier_bin1::deserialize::<
										glacier_bin1::game::$game::STemplateEntityBlueprint
									>(&vanilla_blu_data)?;

									let vanilla_entity = Entity::from_game(
										&vanilla_fac,
										&vanilla_fac_meta,
										&vanilla_blu,
										&vanilla_blu_meta,
										true
									)
									.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

									let patch = generate_patch(&vanilla_entity, &entity)
										.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

									world.remove_mod_file(&node.attribution.mod_id, file_path)?;
									world.remove_mod_file(&node.attribution.mod_id, &metadata_path)?;
									world.remove_mod_file(&node.attribution.mod_id, tblu_path)?;
									world.remove_mod_file(&node.attribution.mod_id, &tblu_metadata_path)?;

									world.write_mod_file(
										&node.attribution.mod_id,
										&file_path.with_file_name(format!("{}.entity.patch.json", id.id.to_hash())),
										&serde_json::to_vec(&patch)?
									)?;

									need_reanalysis = true;
								}
							}
						}};
					}

					match game.version {
						GlacierGame::H1 => impl_game!(h1, STemplateEntity),
						GlacierGame::H2 => impl_game!(h2, STemplateEntityFactory),
						GlacierGame::H3 => impl_game!(h3, STemplateEntityFactory),
						GlacierGame::FL => impl_game!(fl, STemplateEntityFactory)
					}
				}

				// Remove redundant packagedefinition entries from manifest
				if let Operation::AddPackageDefinitionEntry(graph::AddPackageDefinitionEntry { data: entry }) =
					&**operation && deploy_graph.nodes().values().any(|other| match &other.operation {
					NodeOperation::Uncached(op) => match &**op {
						Operation::OverwriteEntity(graph::OverwriteEntity { data, .. }) => {
							data.factory.get_path().is_some_and(|x| x == *entry.path)
						}

						Operation::ApplyQuickEntityPatch(graph::ApplyQuickEntityPatch { data, .. }) => {
							data.factory.get_path().is_some_and(|x| x == *entry.path)
						}

						_ => false
					},

					_ => false
				}) {
					world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::ApplyingMigration {
							message: format!(
								"A packagedefinition entry is redundant due to automatic detection and will be \
								 removed from the manifest: {}",
								entry.path
							)
						},
						target: node.attribution.to_diagnostic_target()
					})?;

					let mut manifest = (*world.get_mod_manifest(&node.attribution.mod_id)?).to_owned();

					manifest.data.package_definition.retain(|x| x.path != entry.path);

					fn recurse_option(path: &NonEmptyString, option: &mut ModOptionData) {
						match option {
							ModOptionData::Boolean { data, .. }
							| ModOptionData::Number { data, .. }
							| ModOptionData::Color { data, .. }
							| ModOptionData::String { data, .. }
							| ModOptionData::Conditional { data, .. } => {
								data.package_definition.retain(|x| x.path != *path);
							}

							ModOptionData::Selection { options, .. } => {
								for option in options.iter_mut() {
									option.data.package_definition.retain(|x| x.path != *path);
								}
							}

							ModOptionData::OptionGroup { data, options, .. } => {
								data.package_definition.retain(|x| x.path != *path);
								options.iter_mut().for_each(|opt| recurse_option(path, &mut opt.data));
							}
						}
					}

					manifest
						.options
						.iter_mut()
						.for_each(|opt| recurse_option(&entry.path, &mut opt.data));

					world.write_mod_manifest(&node.attribution.mod_id, manifest)?;

					need_reanalysis = true;
				}

				// SDEF -> sounddefs.patch.json
				if let Operation::OverwriteRawResource(graph::OverwriteRawResource { id, data }) = &**operation
					&& data.metadata.resource_type == "SDEF"
					&& game.vanilla_resources.contains_key(&id.id)
				{
					let file_path = &node.attribution.source;

					world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::ApplyingMigration {
							message: format!(
								"This mod overwrites a base game sound definitions file ({id:?}) with a raw resource. \
								 This will be converted into a patch."
							)
						},
						target: node.attribution.to_diagnostic_target()
					})?;

					let mut sdef = SoundDefinitions::parse(game.version, &data.data, &data.metadata)
						.wrap_err("Couldn't parse SDEF file")?;

					let existing = game.infer_resource_specifier(id.id)?.unwrap();

					let existing_partition = game
						.game_files
						.partitions
						.iter()
						.find(|p| p.partition_info().name.as_deref() == Some(existing.partition.as_str()))
						.unwrap();

					let vanilla_sdef = SoundDefinitions::parse(
						game.version,
						&existing_partition.read_resource(&game.to_rrid(id.id))?,
						&existing_partition.get_resource_info(&game.to_rrid(id.id))?.try_into()?
					)
					.wrap_err("Couldn't parse SDEF file")?;

					sdef.name = sdef.name.filter(|x| !vanilla_sdef.name.is_some_and(|y| *x == y));

					sdef.definitions
						.retain(|k, v| vanilla_sdef.definitions.get(k) != Some(v));

					if sdef.name.is_some() || !sdef.definitions.is_empty() {
						let new_path = file_path.with_extension("sounddefs.patch.json");
						world.write_mod_file(&node.attribution.mod_id, &new_path, &serde_json::to_vec(&sdef)?)?;
					}

					world.remove_mod_file(&node.attribution.mod_id, &file_path)?;
					world.remove_mod_file(
						&node.attribution.mod_id,
						&file_path.with_extension("SDEF.metadata.json")
					)?;

					need_reanalysis = true;
				}
			}
		}

		if need_reanalysis {
			(*deploy_graph, *peacock_plugins, *sdk_mods, *resources_to_port) = analyse(config, world, game, cache)?;
			continue;
		} else {
			break;
		}
	}
}

#[try_fn]
#[instrument(skip(cache, state, node, reverse_dependencies))]
pub async fn apply_graph_node(
	cache: Arc<Cache>,
	state: Arc<State<impl World + Mods + Progress>>,
	id: EcoString,
	node: GraphNode,
	reverse_dependencies: Vec<Vec<ResourceSpecifier>>
) -> Result<EcoString> {
	let attribution = node.attribution.to_owned();

	let resources_to_possibly_stage = node.operation.resource_deps((&*state.game).into());

	let warn_on_identical = node.operation.warn_on_identical();

	let mutations = node.evaluate(&cache, &state, &id).await.wrap_err_with(|| {
		format!(
			"Couldn't apply graph operation {} - {}",
			attribution.mod_id, attribution.source
		)
	})?;

	for mutation in mutations {
		state.apply(&attribution, mutation, warn_on_identical).await?;
	}

	for resource in resources_to_possibly_stage {
		if !reverse_dependencies
			.iter()
			.any(|resources| resources.contains(&resource))
		{
			// This operation is last in its "chain" in the graph for this resource, so we can stage it
			state.stage(resource).await?;
		}
	}

	id
}

#[derive(Clone)]
pub struct DeployWorld {
	pub mods: Filesystem,
	pub output: GameOutput,
	pub diagnostics: LogDiagnostics,
	pub progress: Arc<dyn Progress + Send + Sync + 'static>
}

impl Mods for DeployWorld {
	delegate! {
		to self.mods {
			fn get_all_mods(&self) -> Result<Vec<ModID>>;
			fn get_mod_root(&self, id: &ModID) -> Option<PathBuf>;
			fn get_mod_manifest(&self, id: &ModID) -> Result<Arc<Manifest>>;
			fn read_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<Vec<u8>>;
			fn read_mod_content_folder(
				&self,
				id: &ModID,
				content_folder: &RelativePath
			) -> Result<Vec<(NominalPartition, Vec<RelativePathBuf>)>>;
			fn read_mod_blob_folder(
				&self,
				id: &ModID,
				blob_folder: &RelativePath
			) -> Result<Vec<(String, RelativePathBuf)>>;
		}
	}
}

impl ModsWritable for DeployWorld {
	delegate! {
		to self.mods {
			fn write_mod_manifest(&self, id: &ModID, manifest: Manifest) -> Result<()>;
			fn write_mod_file(&self, id: &ModID, path: &RelativePath, data: &[u8]) -> Result<()>;
			fn remove_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<()>;
		}
	}
}

impl Output for DeployWorld {
	delegate! {
		to self.output {
			fn emit_sdk_mod(&self, name: String, path: PathBuf) -> Result<()>;
			fn emit_package_definition(&self, package_definition: String) -> Result<()>;
			fn emit_thumbs(&self, thumbs: String) -> Result<()>;
			fn emit_rpkg(&self, package: (PartitionId, PatchId), contents: &[u8]) -> Result<()>;
		}
	}
}

impl Diagnostics for DeployWorld {
	delegate! {
		to self.diagnostics {
			fn emit_diagnostic(&self, diagnostic: Diagnostic) -> Result<()>;
		}
	}
}

impl Progress for DeployWorld {
	delegate! {
		to self.progress {
			fn start_progress(&self, name: &str, max: u64) -> Result<u64>;
			fn start_spinner(&self, prefix: &str, target: Option<&str>, name: &str) -> Result<u64>;
			fn advance_progress(&self, id: u64, amount: u64) -> Result<()>;
			fn finish_progress(&self, id: u64) -> Result<()>;
		}
	}
}

/// Generate and emit RPKGs for all staged resources and ported resources.
/// Will emit AT the patch level of each partition specified by `partitions`; ensure this is computed from a patched packagedefinition.
#[try_fn]
#[wrap_err("Couldn't generate RPKGs")]
#[instrument(skip_all)]
pub fn generate_rpkgs<W: World + Progress + Output>(
	world: &W,
	game: &GameContext,
	partitions: Vec<PartitionInfo>,
	staged: HashMap<EcoString, Vec<ResourceState>>,
	mut port_builders: HashMap<EcoString, Vec<PackageResourceBuilder>>
) -> Result<()> {
	let progress = world.start_progress(
		"Generating RPKGs",
		staged.keys().chain(port_builders.keys()).unique().count() as u64
	)?;

	staged
		.into_iter()
		.map(|(partition, mut staged)| {
			let ports = port_builders.remove(&partition).unwrap_or_default();

			// Enforce a consistent order to fix a very strange non-determinism affecting UI icons
			staged.sort_unstable_by_key(|x| x.metadata.id);

			(partition, staged, ports)
		})
		.collect_vec()
		.into_iter()
		.chain(
			port_builders
				.into_iter()
				.map(|(partition, ports)| (partition, vec![], ports))
		)
		.par_bridge()
		.try_for_each(|(partition_name, resources, ports)| {
			let _span = tracing::info_span!("Generating patch for partition", ?partition_name).entered();

			if !ports.is_empty() || !resources.is_empty() {
				let spinner = world.start_spinner("Generating patch for", None, &partition_name)?;

				let Some(partition_info) = partitions
					.iter()
					.find(|x| x.name.as_deref().is_some_and(|y| y == partition_name))
				else {
					bail!("No such partition {partition_name}")
				};

				let builder = {
					let _span = tracing::info_span!("Generating builder").entered();

					let mut builder = PackageBuilder::new_with_patch_id(
						partition_info.id.to_owned(),
						PatchId::Patch(partition_info.patch_level),
						game.version.into()
					);

					builder.with_resources(ports);

					if Arguments::from_env().contains("--emit-unpacked") {
						let partition_folder = Path::new("unpacked").join(&partition_name);
						fs::create_dir_all(&partition_folder)?;
						for res in &resources {
							fs::write(
								partition_folder.join(format!(
									"{}.{}",
									res.metadata.id.to_hash(),
									res.metadata.resource_type
								)),
								&res.data
							)?;
							fs::write(
								partition_folder.join(format!(
									"{}.{}.metadata.json",
									res.metadata.id.to_hash(),
									res.metadata.resource_type
								)),
								serde_json::to_vec_pretty(&res.metadata)?
							)?;
						}
					}

					builder.with_resources(
						resources
							.into_par_iter()
							.map(|res| {
								let id = res.metadata.id;
								res.into_builder(game)
									.wrap_err_with(|| format!("Couldn't finalise resource {id}"))
							})
							.collect::<Result<Vec<_>>>()?
					);

					builder
				};

				{
					let _span = tracing::info_span!("Building RPKG").entered();

					world.emit_rpkg(
						(partition_info.id.to_owned(), PatchId::Patch(partition_info.patch_level)),
						&builder
							.build_to_vec(if game.version == GlacierGame::H3 || game.version == GlacierGame::FL {
								PackageVersion::RPKGv2
							} else {
								PackageVersion::RPKGv1
							})
							.wrap_err_with(|| format!("Couldn't build RPKG for {partition_name}"))?
					)?;
				}

				world.finish_progress(spinner)?;
			}

			world.advance_progress(progress, 1)?;

			color_eyre::eyre::Ok(())
		})
		.wrap_err("Couldn't generate RPKGs")?;

	world.finish_progress(progress)?;
}

#[try_fn]
#[wrap_err("Couldn't port resources")]
#[instrument(skip_all)]
pub fn port_resources<W: Progress + Send + Sync>(
	world: &W,
	game: &GameContext,
	deployment: &DeployContext
) -> Result<HashMap<EcoString, Vec<PackageResourceBuilder>>> {
	let resources_to_port = deployment.resources_to_port.pin_owned();
	let resources_to_port_count = resources_to_port.len();

	if resources_to_port_count != 0 {
		let progress = world.start_progress("Gathering dependencies", resources_to_port_count as u64)?;

		let partition_hierarchies = resources_to_port
			.iter()
			.map(|(_, for_partition)| for_partition)
			.unique()
			.map(|for_partition| Ok((for_partition.to_owned(), game.get_accessible_partitions(for_partition)?)))
			.collect::<Result<HashMap<_, _>>>()?;

		let to_port: HashMap<RealPartition, PapayaSet<RuntimeID, BuildIdentityHasher<u64>>> = partition_hierarchies
			.keys()
			.cloned()
			.map(|p| (p, PapayaSet::default()))
			.collect();

		let idx = AtomicU64::new(0);
		let last_idx = AtomicU64::new(0);
		let increment = (resources_to_port_count / 100).max(1) as u64;

		resources_to_port
			.iter()
			.par_bridge()
			.try_for_each(|(resource, for_partition)| {
				let to_port = to_port.get(for_partition).unwrap();
				get_portable_references(
					game,
					&deployment.all_relevant_resources,
					&deployment.resource_states,
					*resource,
					partition_hierarchies.get(for_partition).unwrap(),
					(to_port, &to_port.owned_guard())
				)?;

				let idx_val = idx.fetch_add(1, Ordering::AcqRel) + 1;
				if idx_val.is_multiple_of(increment) {
					world.advance_progress(progress, idx_val.saturating_sub(last_idx.load(Ordering::Acquire)))?;
					last_idx.store(idx_val, Ordering::Release);
				}

				color_eyre::eyre::Ok(())
			})?;

		world.finish_progress(progress)?;

		let resources_to_port_count = to_port.values().map(|x| x.len()).sum::<usize>();

		let progress = world.start_progress("Porting resources", resources_to_port_count as u64)?;

		let idx = AtomicU64::new(0);
		let last_idx = AtomicU64::new(0);
		let increment = (resources_to_port_count / 100).max(1) as u64;

		let builders = to_port
			.into_par_iter()
			.map(|(partition, resources)| {
				let resources = resources
					.pin_owned()
					.iter()
					.enumerate()
					.par_bridge()
					.map(|(i, &resource)| {
						let res = port(
							game,
							&deployment.all_relevant_resources,
							&deployment.resource_states,
							resource
						);

						if i.is_multiple_of(increment as usize) {
							let idx_val = idx.fetch_add(increment, Ordering::AcqRel) + increment;
							world
								.advance_progress(progress, idx_val.saturating_sub(last_idx.load(Ordering::Acquire)))?;
							last_idx.store(idx_val, Ordering::Release);
						}

						res
					})
					.collect::<Result<Vec<_>>>()?;

				let inc = resources.len() as u64 % increment;
				let idx_val = idx.fetch_add(inc, Ordering::AcqRel) + inc;
				world.advance_progress(progress, idx_val.saturating_sub(last_idx.load(Ordering::Acquire)))?;
				last_idx.store(idx_val, Ordering::Release);

				Ok((partition.0, resources))
			})
			.collect::<Result<HashMap<_, _>>>()?;

		world.finish_progress(progress)?;

		builders
	} else {
		HashMap::default()
	}
}

/// Get the recursive references of a given resource, including the resource itself, which are not accessible for the given partition list in the vanilla game.
#[try_fn]
#[wrap_err("Couldn't get recursive references of resource {resource}")]
pub fn get_portable_references<'guard, G: Guard + Sync>(
	game: &GameContext,
	all_relevant_resources: &HashMap<RuntimeID, Vec<RealPartition>, BuildIdentityHasher<u64>>,
	resource_states: &HashMap<ResourceSpecifier, Arc<tokio::sync::Mutex<ResourceStates>>>,
	resource: RuntimeID,
	accessible_partitions: &[RealPartition],
	to_port: (&PapayaSet<RuntimeID, BuildIdentityHasher<u64>>, &'guard G)
) -> Result<()> {
	let (to_port, guard) = to_port;

	// Short-circuit if the resource is already being ported
	if to_port.contains(&resource, guard) {
		return Ok(());
	}

	// Short-circuit if the resource is already staged and accessible
	if let Some(partitions) = all_relevant_resources.get(&resource)
		&& partitions.iter().any(|x| accessible_partitions.contains(x))
	{
		return Ok(());
	}

	// Port from elsewhere in vanilla game
	if let Some(partitions) = game.vanilla_resources.get(&resource)
		&& !partitions.is_empty()
	{
		// Resource exists in vanilla but might not be accessible
		if !partitions.iter().any(|x| accessible_partitions.contains(x)) {
			let rrid = game.to_rrid(resource);

			// Find the first partition which contains the resource
			let partition = game
				.game_files
				.partitions
				.iter()
				.find(|p| p.partition_info().name.as_deref().unwrap() == *partitions[0])
				.unwrap();

			let res_info = partition.get_resource_info(&rrid)?;

			to_port.insert(resource, guard);
			res_info.references().par_iter().try_for_each(|&(reference, _)| {
				get_portable_references(
					game,
					all_relevant_resources,
					resource_states,
					reference.try_into()?,
					accessible_partitions,
					(to_port, guard)
				)
			})?;

			return Ok(());
		} else {
			return Ok(());
		}
	}

	// Port from deleted resources
	if let Some(partitions) = game.deleted_resources.get(&resource)
		&& !partitions.is_empty()
	{
		// Find the first partition which contains the resource as a deleted resource
		let partition = game
			.game_files
			.partitions
			.iter()
			.find(|p| p.partition_info().name.as_deref().unwrap() == *partitions[0])
			.unwrap();

		log::trace!(
			"Porting deleted resource {} from partition {}",
			resource,
			partition.partition_info().name.as_deref().unwrap_or_default()
		);

		let rrid: RuntimeResourceID = game.to_rrid(resource);
		let (res_info, _) = partition
			.removed_resources()
			.into_iter()
			.find(|(x, _)| *x.rrid() == rrid)
			.unwrap();

		to_port.insert(resource, guard);
		res_info.references().par_iter().try_for_each(|&(reference, _)| {
			get_portable_references(
				game,
				all_relevant_resources,
				resource_states,
				reference.try_into()?,
				accessible_partitions,
				(to_port, guard)
			)
		})?;

		return Ok(());
	}

	// Port from staging (might be new from a mod)
	if let Some(partitions) = all_relevant_resources.get(&resource)
		&& !partitions.is_empty()
	{
		to_port.insert(resource, guard);

		let refs = futures::executor::block_on(
			resource_states
				.get(&ResourceSpecifier {
					id: resource,
					partition: partitions[0].to_owned()
				})
				.unwrap()
				.lock()
		)
		.back()
		.unwrap()
		.1
		.metadata
		.references
		.to_owned();

		refs.into_par_iter().try_for_each(|reference| {
			get_portable_references(
				game,
				all_relevant_resources,
				resource_states,
				reference.resource,
				accessible_partitions,
				(to_port, guard)
			)
		})?;

		return Ok(());
	}
}

#[try_fn]
#[wrap_err("Couldn't get resource {resource} for porting")]
pub fn port(
	game: &GameContext,
	all_relevant_resources: &HashMap<RuntimeID, Vec<RealPartition>, BuildIdentityHasher<u64>>,
	resource_states: &HashMap<ResourceSpecifier, Arc<tokio::sync::Mutex<ResourceStates>>>,
	resource: RuntimeID
) -> Result<PackageResourceBuilder> {
	if let Some(partitions) = game.vanilla_resources.get(&resource)
		&& !partitions.is_empty()
	{
		// Find the first partition which contains the resource
		for partition in &game.game_files.partitions {
			let rrid = game.to_rrid(resource);

			if partition.contains(&rrid) {
				// Port the modded version of the resource if possible
				// Avoids situations where a modded copy is overwritten by a vanilla resource being ported to a lower chunk
				if let Some(staged) = resource_states.get(&ResourceSpecifier {
					id: resource,
					partition: partitions[0].to_owned()
				}) {
					return futures::executor::block_on(staged.lock())
						.back()
						.unwrap()
						.1
						.to_owned()
						.into_builder(game);
				} else {
					let (res_info, res_data) = (
						partition.get_resource_info(&rrid)?,
						partition
							.read_resource(&rrid)
							.wrap_err("Couldn't extract ported resource")?
					);

					let mut resource = PackageResourceBuilder::from_memory(
						*res_info.rrid(),
						&res_info.data_type(),
						res_data,
						(res_info.is_compressed() && res_info.size() > 1024).then_some(1),
						res_info.is_scrambled()
					)
					.wrap_err("Couldn't construct resource builder")?;

					resource.with_memory_requirements(
						res_info.system_memory_requirement(),
						res_info.video_memory_requirement()
					);

					resource.with_references(res_info.references());

					return Ok(resource);
				};
			}
		}
	}

	// Check if the resource is deleted in any partition
	if let Some(partitions) = game.deleted_resources.get(&resource)
		&& !partitions.is_empty()
	{
		// Find the first partition which contains the resource as a deleted resource
		let partition = game
			.game_files
			.partitions
			.iter()
			.find(|p| p.partition_info().name.as_deref().unwrap() == *partitions[0])
			.unwrap();

		let rrid: RuntimeResourceID = game.to_rrid(resource);
		let (res_info, patch_id) = partition
			.removed_resources()
			.into_iter()
			.find(|(x, _)| *x.rrid() == rrid)
			.unwrap();

		// Port the modded version of the resource if possible
		if let Some(staged) = resource_states.get(&ResourceSpecifier {
			id: resource,
			partition: RealPartition(
				partition
					.partition_info()
					.name
					.as_deref()
					.ok_or_eyre("No partition name")?
					.into()
			)
		}) {
			return futures::executor::block_on(staged.lock())
				.back()
				.unwrap()
				.1
				.to_owned()
				.into_builder(game);
		} else {
			let res_data = partition.packages[&patch_id]
				.read_resource(&rrid)
				.wrap_err("Couldn't extract deleted resource from patch")?;

			let mut resource = PackageResourceBuilder::from_memory(
				*res_info.rrid(),
				&res_info.data_type(),
				res_data,
				(res_info.is_compressed() && res_info.size() > 1024).then_some(1),
				res_info.is_scrambled()
			)
			.wrap_err("Couldn't construct resource builder")?;

			resource.with_memory_requirements(
				res_info.system_memory_requirement(),
				res_info.video_memory_requirement()
			);

			resource.with_references(res_info.references());

			return Ok(resource);
		}
	}

	// The resource must be in staging
	futures::executor::block_on(
		resource_states
			.get(&ResourceSpecifier {
				id: resource,
				partition: all_relevant_resources[&resource][0].to_owned()
			})
			.unwrap()
			.lock()
	)
	.back()
	.unwrap()
	.1
	.to_owned()
	.into_builder(game)?
}
