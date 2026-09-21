use std::{
	fs,
	hash::{Hash, Hasher},
	ops::Deref,
	path::PathBuf,
	str::FromStr,
	sync::Arc
};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use color_eyre::eyre::{OptionExt, Result, WrapErr, bail, eyre};
use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_commons::{
	game::GlacierGame,
	metadata::{ResourceMetadata, RuntimeID}
};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use relative_path::RelativePath;
use semver::Version;
use serde_json::from_slice;
use simple_mod_framework_core::{
	game::{
		GameContext, NominalPartition, REPO_ID, RealPartition, ResourceSpecifier, UNLOCKABLES_ID_FL, UNLOCKABLES_ID_WOA
	},
	intentional_halt,
	utils::ResultExt
};
use simple_mod_framework_types::{
	Config, HashMap, HashSet, ManifestConditions, ManifestData, ModID, ModOption, ModOptionData, ModOptionID,
	ModOptionValue, NumberOptionValidation, PortedResource, SelectionOption, ValidationResult, VersionPlatform
};
use tracing::instrument;
use tryvial::{try_block, try_fn};
use velcro::vec;
use xxhash_rust::xxh3::Xxh3Default;

use crate::{
	APP_VERSION,
	cache::Cache,
	diagnostics::{Diagnostic, DiagnosticKind, DiagnosticTarget},
	graph::{self, Attribution, DeployGraph, GraphOperation, Operation},
	scripts::{ConditionContext, eval_condition, eval_script_data, eval_script_operations},
	state::ResourceState,
	world::{Diagnostics, Mods}
};

#[try_fn]
#[wrap_err("Couldn't analyse mods")]
#[instrument(skip_all)]
pub fn analyse(
	config: &Arc<Config>,
	world: &Arc<impl Mods + Diagnostics + Send + Sync + 'static>,
	game: &Arc<GameContext>,
	cache: &Cache
) -> Result<(
	DeployGraph,
	Vec<PathBuf>,
	Vec<(ModID, PathBuf)>,
	HashSet<(RuntimeID, RealPartition)>
)> {
	let mut graph = DeployGraph::new(game.deref());
	let mut peacock_plugins = vec![];
	let mut sdk_mods = vec![];
	let mut resources_to_port = HashSet::default();

	log::info!("Analysing mods");

	let default_mod_options = HashMap::default();

	let enabled_mod_versions: Arc<HashMap<ModID, Version>> = config
		.deploy_order
		.iter()
		.map(|mod_id| Ok((mod_id.to_owned(), world.get_mod_manifest(mod_id)?.version.0.to_owned())))
		.collect::<Result<HashMap<_, _>>>()?
		.into();

	for mod_id in &config.deploy_order {
		let _span = tracing::info_span!("Analysing mod", ?mod_id).entered();

		let selected_mod_options = config.mod_options.get(mod_id).unwrap_or(&default_mod_options);

		let mut manifest = (*world.get_mod_manifest(mod_id)?).to_owned();

		log::debug!(target: manifest.name.loc(&config.ui_locale), "Analysing mod");

		let x: Result<_> = try_block! {
			if manifest.framework_version.major != APP_VERSION.major {
				intentional_halt!(
					manifest.name.loc(&config.ui_locale),
					format!(
						"This mod is designed for a different major version ({}) of the framework ({}) and is \
						 incompatible!",
						manifest.framework_version, *APP_VERSION
					)
				);
			}

			if *manifest.framework_version > *APP_VERSION {
				intentional_halt!(
					manifest.name.loc(&config.ui_locale),
					format!(
						"This mod is designed for a newer version ({}) of the framework ({}) and is incompatible!",
						manifest.framework_version, *APP_VERSION
					)
				);
			}

			let mut mod_options = HashMap::default();
			for option in &manifest.options {
				get_effective_option_values(world, (&**game).into(), config, &enabled_mod_versions, selected_mod_options, &mut mod_options, option)?;
			}

			let mut option_values = mod_options
				.iter()
				.map(|(x, y)| {
					color_eyre::eyre::Ok(match y {
						ModOptionValue::Boolean { value } => vec![(x.to_owned(), value.to_string(), true)],

						ModOptionValue::Number { value } => vec![(
							x.to_owned(),
							if matches!(
								ModOption::get_option_by_id(&manifest.options, x),
								Some(ModOption {
									data: ModOptionData::Number {
										validation: NumberOptionValidation { integer, .. },
										..
									},
									..
								}) if *integer
							) {
								(value.round() as i64).to_string()
							} else {
								value.to_string()
							},
							true
						)],

						ModOptionValue::Color { value } => vec![(x.to_owned(), value.to_string(), false)],

						ModOptionValue::String { value } => vec![(x.to_owned(), value.to_string(), false)],

						// Selection option gets expanded to value ID, suboptions get expanded to true/false as whether they are selected
						ModOptionValue::Selection { value } => [(x.to_owned(), value.to_string(), false)]
							.into_iter()
							.chain({
								let ModOption { data: ModOptionData::Selection { options, .. }, .. } =
									ModOption::get_option_by_id(&manifest.options, x).ok_or_eyre("No such option")?
								else {
									bail!("Option is not a selection option");
								};

								options.iter().map(|x| (x.id.to_owned(), (*value == x.id).to_string(), true))
							})
							.collect()
					})
				})
				.collect::<Result<Vec<_>>>()?
				.into_iter()
				.flatten()
				.collect_vec();

			option_values.extend(
				manifest.options.iter().map(|opt| {
					let id = &opt.id;
					match &opt.data {
						ModOptionData::Conditional { condition, .. } => Ok(Some((
							id.to_owned(),
							eval_condition(
								id.as_str(),
								condition.as_str(),
								enabled_mod_versions.clone(),
								ConditionContext {
									platform: game.deref().into(),
									config: (**config).to_owned()
								}
							)
							.wrap_err_with(|| eyre!("Couldn't check option {id}'s condition"))?
							.to_string(),
							true
						))),

						ModOptionData::OptionGroup { display_condition, .. } => Ok(Some((
							id.to_owned(),
							if let Some(display_condition) = display_condition {
								eval_condition(
									id.as_str(),
									display_condition.as_str(),
									enabled_mod_versions.clone(),
									ConditionContext {
										platform: game.deref().into(),
										config: (**config).to_owned()
									}
								)
								.wrap_err_with(|| eyre!("Couldn't check option {id}'s display condition"))?
								.to_string()
							} else {
								"true".into()
							},
							true
						))),

						_ => Ok(None)
					}
				})
				.flat_map(Result::transpose)
				.collect::<Result<Vec<_>>>()?
			);

			let mut searchers = option_values
				.iter()
				.flat_map(|(x, _, _)| {
					[
						format!(r##"#{{option-raw:{}}}"##, x),
						format!(r##""#{{option:{}}}""##, x),
						format!(r##"#{{option:{}}}"##, x)
					]
				})
				.collect::<Vec<_>>();

			let mut replacers = option_values
				.iter()
				.flat_map(|(_, y, remove_quotes)| [
					y.to_owned(),
					if *remove_quotes {
						y.to_owned()
					} else {
						format!(r#""{y}""#)
					},
					y.to_owned()
				])
				.collect::<Vec<_>>();

			// To ensure analysis caching doesn't break when substitutions change
			let substitutions_hash = {
				let mut hasher = Xxh3Default::new();
				option_values.hash(&mut hasher);
				hasher.finish()
			};

			// Effective deploy order: base manifest followed by options in the order listed in the manifest
			for option in manifest.options {
				merge_option_data(world, game.deref().into(), config, &enabled_mod_versions, &mod_options, &mut manifest.data, option)?;
			}

			for script in manifest.data.scripts.clone() {
				let script_contents = String::from_utf8(
					world
						.read_mod_file(mod_id, &script)
						.wrap_err("Couldn't read script file")?
				)?;

				if script_contents.contains("pub fn data(") {
					log::debug!(target: manifest.name.loc(&config.ui_locale), "Getting data from script {}", script.as_str());
					let substitutions = eval_script_data(
						config.clone(),
						world.clone(),
						game.clone(),
						Attribution {
							mod_id: mod_id.to_owned(),
							source: script.as_str().into(),
							script_identifier: None
						},
						script_contents,
						&mut manifest.data
					)?;

					for (from, to) in substitutions {
						searchers.push(format!(r##"#{{script-raw:{}}}"##, from));
						searchers.push(format!(r##""#{{script:{}}}""##, from));
						searchers.push(format!(r##"#{{script:{}}}"##, from));

						match to {
							serde_json::Value::String(s) => {
								replacers.push(s.to_owned());
								replacers.push(format!(r#""{s}""#));
								replacers.push(s.to_owned());
							}

							serde_json::Value::Bool(b) => {
								replacers.push(b.to_string());
								replacers.push(b.to_string());
								replacers.push(b.to_string());
							}

							serde_json::Value::Number(n) => {
								replacers.push(n.to_string());
								replacers.push(n.to_string());
								replacers.push(n.to_string());
							}

							serde_json::Value::Array(a) => {
								replacers.push(serde_json::to_string(&a)?);
								replacers.push(serde_json::to_string(&a)?);
								replacers.push(serde_json::to_string(&a)?);
							}

							serde_json::Value::Object(o) => {
								replacers.push(serde_json::to_string(&o)?);
								replacers.push(serde_json::to_string(&o)?);
								replacers.push(serde_json::to_string(&o)?);
							}

							serde_json::Value::Null => {
								replacers.push("null".into());
								replacers.push("null".into());
								replacers.push("null".into());
							}
						}
					}
				}
			}

			let searcher =
				AhoCorasickBuilder::new()
					.match_kind(MatchKind::LeftmostFirst)
					.build(searchers)?;

			manifest.data.content_folders = manifest.data.content_folders.into_iter().unique().collect();
			manifest.data.blob_folders = manifest.data.blob_folders.into_iter().unique().collect();
			manifest.data.package_definition = manifest.data.package_definition.into_iter().unique().collect();
			manifest.data.port_resources = manifest.data.port_resources.into_iter().unique().collect();
			manifest.data.deploy_before = manifest.data.deploy_before.into_iter().unique().collect();
			manifest.data.deploy_after = manifest.data.deploy_after.into_iter().unique().collect();
			manifest.data.peacock_plugins = manifest.data.peacock_plugins.into_iter().unique().collect();
			manifest.data.sdk_mods = manifest.data.sdk_mods.into_iter().unique().collect();
			manifest.data.scripts = manifest.data.scripts.into_iter().unique().collect();

			let script_manifest_data = manifest.data.to_owned();

			log::trace!(target: manifest.name.loc(&config.ui_locale), "Checking platform requirements");
			if !manifest.conditions.supported_games.contains(&(&**game).into()) {
				world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::UnsupportedPlatform {
						platform: (&**game).into(),
						supported: manifest.conditions.supported_games.iter().copied().collect()
					},
					target: DiagnosticTarget::Mod { mod_id: mod_id.clone() }
				})?;
			}

			log::trace!(target: manifest.name.loc(&config.ui_locale), "Checking requirements/incompatibilities");
			if let ValidationResult::Fail(msg) = evaluate_manifest_conditions(
				world.deref(),
				(&**game).into(),
				config,
				&enabled_mod_versions,
				&manifest.conditions,
				manifest.name.loc(&config.ui_locale).as_str()
			)? {
				intentional_halt!(manifest.name.loc(&config.ui_locale), msg);
			}

			for script in manifest.data.scripts.clone() {
				let script_contents = String::from_utf8(
					world
						.read_mod_file(mod_id, &script)
						.wrap_err("Couldn't read script file")?
				)?;

				if script_contents.contains("pub fn operations(") {
					log::debug!(
						target: manifest.name.loc(&config.ui_locale),
						"Getting operations from script {}",
						script.as_str()
					);

					for (id, operation) in eval_script_operations(
						config.clone(),
						world.clone(),
						game.clone(),
						Attribution {
							mod_id: mod_id.to_owned(),
							source: script.as_str().into(),
							script_identifier: None
						},
						script_contents,
						script_manifest_data.to_owned()
					)? {
						let attribution = Attribution {
							mod_id: mod_id.to_owned(),
							source: script.as_str().into(),
							script_identifier: Some(id)
						};

						graph.insert(cache, attribution, None, operation);
					}
				}
			}

			for (content_folder, partitions) in manifest
				.data
				.content_folders
				.into_par_iter()
				.map(|content_folder| {
					let partitions = world.read_mod_content_folder(mod_id, &content_folder)?;
					Ok((content_folder, partitions))
				})
				.collect::<Result<Vec<_>>>()?
			{
				let _span = tracing::info_span!("Analysing content folder", ?content_folder).entered();

				for (partition_folder, files) in partitions {
					if !["h1-", "h2-", "h3-", "fl-"]
						.iter()
						.filter(|&&x| {
							x != match game.version {
								GlacierGame::H1 => "h1-",
								GlacierGame::H2 => "h2-",
								GlacierGame::H3 => "h3-",
								GlacierGame::FL => "fl-"
							}
						})
						.any(|&x| partition_folder.starts_with(x))
					{
						let files = files
							.into_iter()
							.filter(|path| {
								if let Some((_, ty)) = path.file_name().unwrap().split_once('.') {
									ANALYSERS.iter().any(|analyser| analyser.file_type == ty)
										|| (ty.chars().count() == 4 && ty.chars().all(|x| x.is_uppercase()))
								} else {
									false
								}
							})
							.map(|file| Attribution {
								mod_id: mod_id.to_owned(),
								source: file,
								script_identifier: None
							})
							.collect_vec();

						// Identify and read changed files
						let mut changed_files = files
							.par_iter()
							.map(|attr| Ok((attr.to_owned(), {
								let mut hasher = Xxh3Default::new();
								attr.unit_hash(world)?.unwrap_or_else(rand::random).hash(&mut hasher);
								substitutions_hash.hash(&mut hasher);
								hasher.finish()
							})))
							.collect::<Result<Vec<_>>>()?
							.into_par_iter()
							.filter(|(attr, hash)| cache.unit_invalidated(&attr.unit(), *hash))
							.map(|(file, hash)| {
								let operations = get_operations_from_file(
									world,
									game,
									&searcher,
									&replacers,
									mod_id,
									manifest.name.loc(&config.ui_locale),
									&partition_folder,
									&file.source
								)?;

								Ok((file, (hash, operations)))
							})
							.collect::<Result<HashMap<_, _>>>()?;

						let mut need_propagate = false;
						let mut invalidated = vec![];

						// Create skeleton operations in graph
						for file in &files {
							if let Some((hash, operations)) = changed_files.remove(file) {
								let should_cache = operations.iter().all(|op| op.should_cache());

								let nodes = operations
									.into_iter()
									.enumerate()
									.map(|(i, op)| graph.insert_deferring(cache, file.to_owned(), Some(i), op))
									.collect_vec();

								if should_cache {
									cache.insert_unit(file.unit(), hash, nodes.into_iter().map(|x| x.into()).collect());
								}

								need_propagate = true;
							} else {
								for node in cache
									.get_unit(&file.unit())
									.ok_or_eyre("No cached data for unchanged unit")?
									.nodes
								{
									if graph.insert_marker(cache, file.to_owned(), node.into()) {
										need_propagate = true;
										invalidated.push(file.unit());
									}
								}
							}
						}

						if need_propagate {
							invalidated.extend(cache.propagate_invalidations());

							// Read any files now invalidated and populate their operations in the graph
							for (file, operations) in files
								.into_par_iter()
								.filter(|file| invalidated.contains(&file.unit()))
								.map(|file| {
									let operations = get_operations_from_file(
										world,
										game,
										&searcher,
										&replacers,
										mod_id,
										manifest.name.loc(&config.ui_locale),
										&partition_folder,
										&file.source
									)?;

									Ok((file, operations))
								})
								.collect::<Result<Vec<_>>>()?
							{
								for (i, operation) in operations.into_iter().enumerate() {
									graph.modify(cache, &file, i, operation);
								}
							}
						}
					}
				}
			}

			for blob_folder in manifest.data.blob_folders {
				let _span = tracing::info_span!("Analysing blob folder", ?blob_folder).entered();

				let blobs = world
					.read_mod_blob_folder(mod_id, &blob_folder)?
					.into_par_iter()
					.map(|(blob_path, blob_file_path)| {
						let x: Result<_> = try {
							let blob_hash = match &blob_path {
								x if x.ends_with(".png") || x.ends_with(".jpeg") || x.ends_with(".jpg") => {
									RuntimeID::from_path(&format!(
										"[assembly:/{}/online/default/cloudstorage/resources/{blob_path}].{}gfx",
										if game.version == GlacierGame::FL { "_knt" } else { "_pro" },
										if game.version != GlacierGame::FL { "pc_" } else { "" }
									))
								}

								x if x.ends_with(".json") => RuntimeID::from_path(&format!(
									"[assembly:/{}/online/default/cloudstorage/resources/{}].{}{}",
									if game.version == GlacierGame::FL { "_knt" } else { "_pro" },
									blob_path,
									if game.version != GlacierGame::FL { "pc_" } else { "" },
									x.split('.')
										.next_back()
										.ok_or_eyre("Filename must include an extension")
										.intentional()?
								)),

								x if x.ends_with(".xml") => {
									RuntimeID::from_path(&format!(
										"[assembly:/{}/online/default/cloudstorage/resources/{blob_path}].bechaml",
										if game.version == GlacierGame::FL { "_knt" } else { "_pro" },
									))
								}

								x if x.ends_with(".usm") => {
									RuntimeID::from_path(&format!(
										"[assembly:/{}/online/default/cloudstorage/resources/{blob_path}].gfxv",
										if game.version == GlacierGame::FL { "_knt" } else { "_pro" },
									))
								}

								x => {
									Err(eyre!(
										"Unknown blob file type {}",
										x.split('.')
											.next_back()
											.ok_or_eyre("Filename must include an extension")
											.intentional()?
									))?;
									unreachable!();
								}
							};

							let blob_type = match &blob_path {
								x if x.ends_with(".png") || x.ends_with(".jpeg") || x.ends_with(".jpg") => {
									"GFXI".into()
								}

								x if x.ends_with(".xml") => {
									"XMLB".into()
								}

								x if x.ends_with(".usm") => {
									"GFXV".into()
								}

								x => x
									.split('.')
									.next_back()
									.ok_or_eyre("Filename must include an extension")
									.intentional()?
									.to_uppercase()
							};

							let res_type = blob_type
								.parse()
								.wrap_err("File extension must be a valid resource type")
								.intentional()?;

							(
								blob_hash,
								blob_path,
								Attribution {
									mod_id: mod_id.to_owned(),
									source: blob_file_path.to_owned(),
									script_identifier: None
								},
								graph::OverwriteRawResource {
									id: ResourceSpecifier {
										id: blob_hash,
										partition: NominalPartition("super".into())
											.real_candidates(game.deref())
											.unwrap()[0]
											.to_owned()
									},
									data: ResourceState {
										metadata: ResourceMetadata {
											id: blob_hash,
											resource_type: res_type,
											compressed: ResourceMetadata::infer_compressed(res_type),
											scrambled: ResourceMetadata::infer_scrambled(res_type),
											references: vec![]
										},
										data: world.read_mod_file(mod_id, &blob_file_path)?
									}
								}
								.into()
							)
						};

						x.wrap_err_with(|| format!("Couldn't analyse blob file {}", blob_file_path))
					})
					.collect::<Result<Vec<_>>>()?;

				let mut ores_blobs = Vec::with_capacity(blobs.len());
				for (blob_hash, blob_path, attribution, operation) in blobs {
					ores_blobs.push((blob_hash, blob_path));
					graph.insert(cache, attribution, None, operation);
				}

				graph.insert(
					cache,
					Attribution {
						mod_id: mod_id.to_owned(),
						source: blob_folder.as_str().into(),
						script_identifier: None
					},
					None,
					graph::AddBlobsToORES { blobs: ores_blobs }.into()
				);
			}

			{
				let _span = tracing::info_span!("Manifest analysis").entered();

				if !manifest.data.localisation.is_empty() {
					graph.insert(
						cache,
						Attribution {
							mod_id: mod_id.to_owned(),
							source: "manifest.json".into(),
							script_identifier: None
						},
						None,
						graph::AddLocalisation {
							data: manifest.data.localisation.into_iter().collect()
						}
						.into()
					);
				}

				for (line, loc) in manifest.data.localised_lines {
					graph.insert(
						cache,
						Attribution {
							mod_id: mod_id.to_owned(),
							source: "manifest.json".into(),
							script_identifier: None
						},
						None,
						graph::WriteLocalisedLine {
							line: ResourceSpecifier {
								id: line,
								partition: NominalPartition("super".into()).real_candidates(game.deref()).unwrap()[0]
									.to_owned()
							},
							loc: loc.as_str().into()
						}
						.into()
					);
				}

				for data in manifest.data.package_definition {
					graph.insert(
						cache,
						Attribution {
							mod_id: mod_id.to_owned(),
							source: "manifest.json".into(),
							script_identifier: None
						},
						None,
						graph::AddPackageDefinitionEntry { data }.into()
					);
				}

				resources_to_port.extend(
					manifest.data.port_resources.into_iter().flat_map(|data| {
						Some((
							match &data {
								PortedResource::Simple(x) => *x,
								PortedResource::WithOptions { resource, .. } => *resource
							},
							match data {
								PortedResource::Simple(_) => {
									NominalPartition("super".into()).real_candidates(game.deref()).unwrap()[0].to_owned()
								}

								PortedResource::WithOptions { for_partition, .. } => {
									// If the partition to port for doesn't exist, don't port it (? short-circuits and will be flattened)
									NominalPartition(for_partition.as_str().into())
										.real_candidates(game.deref())?
										.first()?
										.to_owned()
								}
							}
						))
					})
				);

				// Only makes sense on filesystem World
				if let Some(mod_root) = world.get_mod_root(mod_id) {
					peacock_plugins.extend(
						manifest
							.data
							.peacock_plugins
							.into_iter()
							.map(|x| fs::canonicalize(x.to_logical_path(&mod_root)))
							.collect::<Result<Vec<_>, _>>()?
					);

					sdk_mods.extend(
						manifest
							.data
							.sdk_mods
							.get(&game.version)
							.unwrap_or(&vec![])
							.iter()
							.map(|x| fs::canonicalize(x.to_logical_path(&mod_root)))
							.collect::<Result<Vec<_>, _>>()?
							.into_iter()
							.map(|x| (mod_id.to_owned(), x))
					);
				}
			}
		};

		x.wrap_err(format!("Couldn't analyse mod {mod_id}"))?
	}

	(graph, peacock_plugins, sdk_mods, resources_to_port)
}

/// Evaluate whether the conditions for a mod or option are met.
#[try_fn]
#[wrap_err("Couldn't evaluate conditions")]
#[instrument(skip_all)]
pub fn evaluate_manifest_conditions(
	world: &impl Mods,
	platform: VersionPlatform,
	config: &Config,
	enabled_mod_versions: &Arc<HashMap<ModID, Version>>,
	manifest_conditions: &ManifestConditions,
	attribution: &str
) -> Result<ValidationResult> {
	if !manifest_conditions.supported_games.is_empty() && !manifest_conditions.supported_games.contains(&platform) {
		return Ok(ValidationResult::Fail(format!(
			"This mod is not compatible with the current platform ({platform})! Supported platforms: {}",
			manifest_conditions
				.supported_games
				.iter()
				.map(|x| x.to_string())
				.collect_vec()
				.join(", ")
		)));
	}

	for requirement in &manifest_conditions.required_mods {
		if !config.deploy_order.contains(&requirement.id) {
			return Ok(ValidationResult::Fail(format!(
				"This mod requires {}, but it isn't enabled!",
				requirement.id
			)));
		}

		let version_loaded = &world.get_mod_manifest(&requirement.id)?.version;
		if !requirement.version.matches(version_loaded) {
			return Ok(ValidationResult::Fail(format!(
				"This mod requires a version of {} matching {}, but version {} was present instead!",
				requirement.id, requirement.version, version_loaded
			)));
		}
	}

	for requirement in &manifest_conditions.required_conditions {
		if !eval_condition(
			attribution,
			requirement.condition.as_str(),
			enabled_mod_versions.clone(),
			ConditionContext {
				platform,
				config: config.to_owned()
			}
		)
		.wrap_err_with(|| eyre!("Couldn't check required condition for {attribution}"))?
		{
			return Ok(ValidationResult::Fail(format!(
				"This mod's requirements are not met! {}",
				requirement.explanation.loc(&config.ui_locale)
			)));
		}
	}

	for incompatibility in &manifest_conditions.incompatible_mods {
		if config.deploy_order.contains(&incompatibility.id) {
			let version_loaded = &world.get_mod_manifest(&incompatibility.id)?.version;
			if incompatibility.version.matches(version_loaded) {
				return Ok(ValidationResult::Fail(format!(
					"This mod is incompatible with versions of {} matching {}, and version {} is present!",
					incompatibility.id, incompatibility.version, version_loaded
				)));
			}
		}
	}

	for incompatibility in &manifest_conditions.incompatible_conditions {
		if eval_condition(
			attribution,
			incompatibility.condition.as_str(),
			enabled_mod_versions.clone(),
			ConditionContext {
				platform,
				config: config.to_owned()
			}
		)
		.wrap_err_with(|| eyre!("Couldn't check incompatible condition for {attribution}"))?
		{
			return Ok(ValidationResult::Fail(format!(
				"This mod is incompatible with the current configuration! {}",
				incompatibility.explanation.loc(&config.ui_locale)
			)));
		}
	}

	ValidationResult::Pass
}

/// Add the effective values of an option and all its sub-options to the map, taking into account defaults and any conditions on the options.
/// Also performs validation of selected option values.
#[try_fn]
#[wrap_err("Couldn't get effective option values")]
#[instrument(skip_all)]
pub fn get_effective_option_values(
	world: &impl Mods,
	platform: VersionPlatform,
	config: &Config,
	enabled_mod_versions: &Arc<HashMap<ModID, Version>>,
	mod_options: &HashMap<ModOptionID, ModOptionValue>,
	effective_values: &mut HashMap<ModOptionID, ModOptionValue>,
	option: &ModOption
) -> Result<()> {
	if let Some(data) = mod_options.get(&option.id)
		&& let ValidationResult::Fail(message) = option.data.validate(data)
	{
		intentional_halt!(format!("Value for option {} is invalid: {}", option.id, message));
	}

	let id = &option.id;

	match &option.data {
		ModOptionData::Boolean {
			default_value,
			conditions,
			..
		} => {
			effective_values.insert(
				id.to_owned(),
				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						conditions,
						id.as_str()
					)?,
					ValidationResult::Pass
				) && let Some(ModOptionValue::Boolean { value }) = mod_options.get(id)
				{
					ModOptionValue::Boolean { value: *value }
				} else {
					ModOptionValue::Boolean { value: *default_value }
				}
			);
		}

		ModOptionData::Selection {
			default_value,
			options,
			conditions,
			..
		} => {
			effective_values.insert(
				id.to_owned(),
				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						conditions,
						id.as_str()
					)?,
					ValidationResult::Pass
				) && let Some(ModOptionValue::Selection { value }) = mod_options.get(id)
					&& let Some(SelectionOption { id, conditions, .. }) = options.iter().find(|x| x.id == *value)
					&& matches!(
						evaluate_manifest_conditions(
							world,
							platform,
							config,
							enabled_mod_versions,
							conditions,
							id.as_str()
						)?,
						ValidationResult::Pass
					) {
					ModOptionValue::Selection {
						value: value.to_owned()
					}
				} else {
					ModOptionValue::Selection {
						value: default_value.to_owned()
					}
				}
			);
		}

		ModOptionData::Conditional { .. } => {}

		ModOptionData::OptionGroup {
			display_condition,
			options,
			..
		} => {
			if display_condition
				.as_ref()
				.map(|display_condition| {
					eval_condition(
						id.as_str(),
						display_condition.as_str(),
						enabled_mod_versions.clone(),
						ConditionContext {
							platform,
							config: config.to_owned()
						}
					)
					.wrap_err_with(|| eyre!("Couldn't check option group {id}'s condition"))
				})
				.unwrap_or(Ok(true))?
			{
				for sub_option in options.iter() {
					get_effective_option_values(
						world,
						platform,
						config,
						enabled_mod_versions,
						mod_options,
						effective_values,
						sub_option
					)?;
				}
			} else {
				// Use default values
				for sub_option in options.iter() {
					get_effective_option_values(
						world,
						platform,
						config,
						enabled_mod_versions,
						&Default::default(),
						effective_values,
						sub_option
					)?;
				}
			}
		}

		ModOptionData::Number {
			default_value,
			conditions,
			..
		} => {
			effective_values.insert(
				id.to_owned(),
				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						conditions,
						id.as_str()
					)?,
					ValidationResult::Pass
				) && let Some(ModOptionValue::Number { value }) = mod_options.get(id)
				{
					ModOptionValue::Number { value: *value }
				} else {
					ModOptionValue::Number { value: *default_value }
				}
			);
		}

		ModOptionData::Color {
			default_value,
			conditions,
			..
		} => {
			effective_values.insert(
				id.to_owned(),
				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						conditions,
						id.as_str()
					)?,
					ValidationResult::Pass
				) && let Some(ModOptionValue::Color { value }) = mod_options.get(id)
				{
					ModOptionValue::Color {
						value: value.to_owned()
					}
				} else {
					ModOptionValue::Color {
						value: default_value.to_owned()
					}
				}
			);
		}

		ModOptionData::String {
			default_value,
			conditions,
			..
		} => {
			effective_values.insert(
				id.to_owned(),
				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						conditions,
						id.as_str()
					)?,
					ValidationResult::Pass
				) && let Some(ModOptionValue::String { value }) = mod_options.get(id)
				{
					ModOptionValue::String {
						value: value.to_owned()
					}
				} else {
					ModOptionValue::String {
						value: default_value.to_owned()
					}
				}
			);
		}
	}
}

/// Recursively merge the data from options into a mod's manifest.
#[try_fn]
#[wrap_err("Couldn't process option data")]
#[instrument(skip_all)]
pub fn merge_option_data(
	world: &impl Mods,
	platform: VersionPlatform,
	config: &Config,
	enabled_mod_versions: &Arc<HashMap<ModID, Version>>,
	mod_options: &HashMap<ModOptionID, ModOptionValue>,
	manifest_data: &mut ManifestData,
	option: ModOption
) -> Result<()> {
	let id = option.id;

	match option.data {
		ModOptionData::Boolean {
			default_value,
			conditions,
			data,
			..
		} => {
			let value = if let Some(ModOptionValue::Boolean { value }) = mod_options.get(&id) {
				*value
			} else {
				default_value
			};

			// Merge with manifest
			if matches!(
				evaluate_manifest_conditions(world, platform, config, enabled_mod_versions, &conditions, id.as_str())?,
				ValidationResult::Pass
			) && value
			{
				manifest_data.extend(data);
			}
		}

		ModOptionData::Selection {
			default_value,
			options,
			conditions,
			data,
			..
		} => {
			let value = if let Some(ModOptionValue::Selection { value }) = mod_options.get(&id) {
				value.to_owned()
			} else {
				default_value
			};

			if matches!(
				evaluate_manifest_conditions(world, platform, config, enabled_mod_versions, &conditions, id.as_str())?,
				ValidationResult::Pass
			) {
				manifest_data.extend(data);

				// Merge with manifest
				let selected_data = options
					.iter()
					.find(|x| x.id == value)
					.ok_or_else(|| eyre!("No such option {} for mod", value))?;

				if matches!(
					evaluate_manifest_conditions(
						world,
						platform,
						config,
						enabled_mod_versions,
						&selected_data.conditions,
						value.as_str()
					)?,
					ValidationResult::Pass
				) {
					manifest_data.extend(selected_data.data.to_owned());
				}
			}
		}

		ModOptionData::Conditional { condition, data } => {
			if eval_condition(
				id.as_str(),
				condition.as_str(),
				enabled_mod_versions.clone(),
				ConditionContext {
					platform,
					config: config.to_owned()
				}
			)
			.wrap_err_with(|| eyre!("Couldn't check conditional option {id}'s condition"))?
			{
				manifest_data.extend(data);
			}
		}

		ModOptionData::OptionGroup {
			display_condition,
			options,
			data,
			..
		} => {
			if display_condition
				.map(|display_condition| {
					eval_condition(
						id.as_str(),
						display_condition.as_str(),
						enabled_mod_versions.clone(),
						ConditionContext {
							platform,
							config: config.to_owned()
						}
					)
					.wrap_err_with(|| eyre!("Couldn't check option group {id}'s condition"))
				})
				.unwrap_or(Ok(true))?
			{
				manifest_data.extend(data);

				for option in options.0 {
					merge_option_data(
						world,
						platform,
						config,
						enabled_mod_versions,
						mod_options,
						manifest_data,
						option
					)?;
				}
			}
		}

		ModOptionData::Number { conditions, data, .. }
		| ModOptionData::Color { conditions, data, .. }
		| ModOptionData::String { conditions, data, .. } => {
			if matches!(
				evaluate_manifest_conditions(world, platform, config, enabled_mod_versions, &conditions, id.as_str())?,
				ValidationResult::Pass
			) {
				manifest_data.extend(data);
			}
		}
	}
}

pub struct Analyser {
	pub file_type: &'static str,

	pub analyse: fn(
		world: &dyn Mods,
		game: &GameContext,
		mod_id: &ModID,
		attribution: &str,
		partition: &EcoString,
		file: &RelativePath,
		file_contents: Vec<u8>
	) -> Result<Vec<Operation>>
}

#[linkme::distributed_slice]
pub static ANALYSERS: [Analyser];

#[try_fn]
#[wrap_err("Couldn't analyse content file {}", file.as_str())]
#[instrument(skip_all)]
fn get_operations_from_file(
	world: &Arc<impl Mods>,
	game: &GameContext,
	searcher: &AhoCorasick,
	replacers: &[String],
	mod_id: &ModID,
	attribution: &str,
	partition: &EcoString,
	file: &RelativePath
) -> Result<Vec<Operation>> {
	let mut file_contents = world.read_mod_file(mod_id, file)?;

	// compat fails early (speeds up handling of large binary non-utf8 files)
	if simdutf8::compat::from_utf8(&file_contents).is_ok() {
		file_contents = searcher.replace_all_bytes(&file_contents, replacers);
	}

	let file_name = file.file_name().ok_or_eyre("No file name")?;

	if let Some((_, file_type)) = file_name.split_once('.') {
		for analyser in ANALYSERS {
			if analyser.file_type == file_type {
				return (analyser.analyse)(world.deref(), game, mod_id, attribution, partition, file, file_contents);
			}
		}
	}

	if let Some((id, resource_type)) = file_name.split_once('.')
		&& resource_type.chars().count() == 4
		&& resource_type.chars().all(|x| x.is_uppercase())
	{
		let resource_type = resource_type
			.try_into()
			.wrap_err("File extension is not a valid resource type")
			.intentional()?;

		if id == REPO_ID.to_hash() {
			intentional_halt!(
				attribution,
				"This mod overwrites the repository file in its entirety. This is not permitted for compatibility \
				 with game updates and other mods. Use a repository.json or JSON.patch.json file instead."
			);
		}

		if id == UNLOCKABLES_ID_WOA.to_hash() || id == UNLOCKABLES_ID_FL.to_hash() {
			intentional_halt!(
				attribution,
				"This mod overwrites the unlockables file in its entirety. This is not permitted for compatibility \
				 with game updates and other mods. Use an unlockables.json or JSON.patch.json file instead."
			);
		}

		if let Some(spec) =
			game.realise_resource_specifier(RuntimeID::from_str(id)?, NominalPartition(partition.to_owned()))?
		{
			vec![
				graph::OverwriteRawResource {
					id: spec,
					data: ResourceState {
						metadata: if let Ok(metadata_contents) =
							world.read_mod_file(mod_id, &file.with_extension(format!("{resource_type}.metadata.json")))
						{
							from_slice(&metadata_contents)
								.wrap_err("metadata.json was not valid resource metadata JSON")
								.intentional()?
						} else {
							ResourceMetadata {
								id: RuntimeID::from_str(id)?,
								resource_type,
								compressed: ResourceMetadata::infer_compressed(resource_type),
								scrambled: ResourceMetadata::infer_scrambled(resource_type),
								references: vec![]
							}
						},
						data: file_contents
					}
				}
				.into(),
			]
		} else {
			log::debug!(
				target: attribution,
				"Skipping content file {} as partition is not valid for current game", file.as_str()
			);
			vec![]
		}
	} else {
		vec![]
	}
}
