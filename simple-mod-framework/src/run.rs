use std::{
	backtrace::Backtrace,
	env::current_dir,
	fs::{self, File},
	io::{IsTerminal, Write},
	ops::Deref,
	path::{Path, PathBuf},
	process::ExitCode,
	sync::Arc,
	time::{Instant, SystemTime, UNIX_EPOCH}
};

use color_eyre::{
	Section,
	eyre::{OptionExt, Result, WrapErr},
	owo_colors::OwoColorize,
	section::PanicMessage
};
use delegate::delegate;
use fern::colors::{Color, ColoredLevelConfig};
use fn_wrap_err::wrap_err;
use glacier_commons::hash_list::HASH_LIST;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use ipc_channel::ipc::IpcSender;
use lazy_regex::regex_replace_all;
use pico_args::Arguments;
use relative_path::{RelativePath, RelativePathBuf};
use rpkg_rs::resource::{pdefs::PartitionId, resource_partition::PatchId};
use serde::{Deserialize, Serialize};
use serde_json::from_slice;
use simple_mod_framework_core::{
	game::{Filesystem, GameContext, GameOutput, NominalPartition, detect_game},
	intentional_halt,
	utils::{IntentionalHalt, format_json}
};
use simple_mod_framework_types::{Config, DeploySummary, Game, HashMap, Manifest, ModID, SemVer, VersionPlatform};
use specta::Type;
use tracing::instrument;
use tracing_chrome::{ChromeLayerBuilder, TraceStyle};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use tryvial::{try_block, try_fn};

use crate::{
	APP_VERSION, EXPERIMENT,
	analysis::analyse,
	analytics::{AnalyticsConfig, AnalyticsDeployResult, AnalyticsSubmission, DeployUnsuccessfulReason},
	cache::Cache,
	deploy::{DeployWorld, migrate, sort_deploy_order},
	diagnostics::{Diagnostic, DiagnosticKind, DiagnosticSeverity, DiagnosticTarget},
	graph::DeployGraph,
	state::{DeployContext, State},
	utils::DEBUG_PROFILE_DIR,
	world::{CliProgress, Diagnostics, LogDiagnostics, LogProgress, Mods, ModsWritable, Output, Progress}
};

struct PanicReporter;

impl PanicMessage for PanicReporter {
	fn display(&self, pi: &std::panic::PanicHookInfo<'_>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// Print panic message.
		let payload = pi
			.payload()
			.downcast_ref::<String>()
			.map(String::as_str)
			.or_else(|| pi.payload().downcast_ref::<&str>().cloned())
			.unwrap_or("<non string panic payload>");

		if !(payload.contains("RecvError") || payload.contains("SendError")) {
			let _: Result<_> = try_block! {
				let config: Config = from_slice(&fs::read("config.json").wrap_err("Couldn't read config.json file")?)
					.wrap_err("Couldn't parse config.json file")?;

				let panic_msg = format!(
					"{} at {}",
					payload,
					if let Some(loc) = pi.location() {
						format!("{}:{}", loc.file(), loc.line())
					} else {
						"<unknown>".into()
					}
				);

				let game = detect_game(config.game_path.to_owned().ok_or_eyre("No game selected")?)?;

				if let Some(game) = game.as_ref()
					&& config.online_services
				{
					drop(tokio::task::spawn(
						AnalyticsSubmission {
							config: AnalyticsConfig {
								skip_intro: config.skip_intro,
								use_alternative_output_directory: config.use_alternative_output_directory.is_some(),
								mod_options: config
									.mod_options
									.clone()
									.into_iter()
									.filter(|(x, _)| config.deploy_order.contains(x))
									.collect(),
								deploy_order: config.deploy_order.to_owned(),
								developer_mode: config.developer_mode,
								ui_locale: config.ui_locale.to_owned(),
								auto_disable_dynres: config.auto_disable_dynres,
								boot_scene: config.boot_scene.is_some()
							},
							mod_versions: config
								.deploy_order
								.iter()
								.map(|mod_id| {
									Ok((
										mod_id.to_owned(),
										Filesystem::new("Mods")?.get_mod_manifest(mod_id)?.version.to_owned()
									))
								})
								.collect::<Result<HashMap<_, _>>>()?,
							version: SemVer((*APP_VERSION).to_owned()),
							experiment: EXPERIMENT.to_owned().map(|x| x.into()),
							game_hash: game.hash.to_owned(),
							platform: game.into(),
							result: AnalyticsDeployResult::Unsuccessful {
								failure_reason: DeployUnsuccessfulReason::Panic,
								errors: vec![panic_msg.to_owned()]
							}
						}
						.submit()
					));
				}

				let mut profile = File::create(DEBUG_PROFILE_DIR.join("panic.txt"))?;
				writeln!(profile, "The framework crashed due to an internal error.")?;
				writeln!(
					profile,
					"The game version was: {}, {}",
					game.as_ref()
						.map(|x| VersionPlatform::from(x).to_string())
						.unwrap_or_else(|| "unavailable".into()),
					game.as_ref().map(|x| x.hash.deref()).unwrap_or("<unavailable>")
				)?;
				writeln!(profile, "The error was: {panic_msg}")?;
				writeln!(profile, "Backtrace:\n{}", Backtrace::force_capture())?;
			};

			writeln!(f)?;

			writeln!(f, "{}", "The framework has crashed due to an internal error!".red())?;

			writeln!(
				f,
				"Please report this issue at https://hitman-resources.netlify.app/issue-report - this should never \
				 happen."
			)?;

			writeln!(f)?;

			write!(f, "Error: {}", payload.cyan())?;
			write!(f, " at ")?;
			if let Some(loc) = pi.location() {
				write!(f, "{}:{}", loc.file().purple(), loc.line().purple())?;
			} else {
				write!(f, "<unknown>")?;
			}
		}

		Ok(())
	}
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DeployMessage {
	Diagnostic {
		data: DiagnosticKind,
		target: DiagnosticTarget,
		severity: DiagnosticSeverity,
		message: String
	},

	ProgressStart {
		name: String,
		max: u64,
		id: u64
	},

	SpinnerStart {
		prefix: String,
		target: Option<String>,
		name: String,
		id: u64
	},

	ProgressAdvance {
		id: u64,
		amount: u64
	},

	ProgressFinish {
		id: u64
	},

	UnrecognisedGameVersion
}

pub struct IPCWrapper<W> {
	pub world: W,
	pub ipc: Option<IpcSender<Vec<u8>>>
}

impl<W: Mods> Mods for IPCWrapper<W> {
	delegate! {
		to self.world {
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

impl<W: ModsWritable> ModsWritable for IPCWrapper<W> {
	delegate! {
		to self.world {
			fn write_mod_manifest(&self, id: &ModID, manifest: Manifest) -> Result<()>;
			fn write_mod_file(&self, id: &ModID, path: &RelativePath, data: &[u8]) -> Result<()>;
			fn remove_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<()>;
		}
	}
}

impl<W: Output> Output for IPCWrapper<W> {
	delegate! {
		to self.world {
			fn emit_sdk_mod(&self, name: String, path: PathBuf) -> Result<()>;
			fn emit_package_definition(&self, package_definition: String) -> Result<()>;
			fn emit_thumbs(&self, thumbs: String) -> Result<()>;
			fn emit_rpkg(&self, package: (PartitionId, PatchId), contents: &[u8]) -> Result<()>;
		}
	}
}

impl<W: Diagnostics> Diagnostics for IPCWrapper<W> {
	fn emit_diagnostic(&self, diagnostic: Diagnostic) -> Result<()> {
		if let Some(ipc) = &self.ipc {
			ipc.send(serde_brief::to_vec(&DeployMessage::Diagnostic {
				data: diagnostic.kind.to_owned(),
				target: diagnostic.target.to_owned(),
				severity: diagnostic.kind.severity(),
				message: format!("{}", diagnostic.kind)
			})?)?;
		}

		self.world.emit_diagnostic(diagnostic)
	}
}

impl<W: Progress> Progress for IPCWrapper<W> {
	#[try_fn]
	fn start_progress(&self, name: &str, max: u64) -> Result<u64> {
		let id = self.world.start_progress(name, max)?;

		if let Some(ipc) = &self.ipc {
			ipc.send(serde_brief::to_vec(&DeployMessage::ProgressStart {
				name: name.to_owned(),
				max,
				id
			})?)?;
		}

		id
	}

	#[try_fn]
	fn start_spinner(&self, prefix: &str, target: Option<&str>, name: &str) -> Result<u64> {
		let id = self.world.start_spinner(prefix, target, name)?;

		if let Some(ipc) = &self.ipc {
			ipc.send(serde_brief::to_vec(&DeployMessage::SpinnerStart {
				prefix: prefix.to_owned(),
				target: target.map(|x| x.to_owned()),
				name: name.to_owned(),
				id
			})?)?;
		}

		id
	}

	fn advance_progress(&self, id: u64, amount: u64) -> Result<()> {
		if let Some(ipc) = &self.ipc {
			ipc.send(serde_brief::to_vec(&DeployMessage::ProgressAdvance { id, amount })?)?;
		}

		self.world.advance_progress(id, amount)
	}

	fn finish_progress(&self, id: u64) -> Result<()> {
		if let Some(ipc) = &self.ipc {
			ipc.send(serde_brief::to_vec(&DeployMessage::ProgressFinish { id })?)?;
		}

		self.world.finish_progress(id)
	}
}

#[try_fn]
#[wrap_err("Couldn't deploy mods")]
#[instrument(skip_all)]
async fn initialise(
	progress: Arc<dyn Progress + Send + Sync>,
	ipc_name: Option<String>
) -> Result<(Cache, State<IPCWrapper<DeployWorld>>, DeployGraph)> {
	log::info!("Started deployment");

	if let Some(ver) = EXPERIMENT {
		log::info!("Running experimental version: {ver}");
	}

	tokio::task::spawn(
		reqwest::Client::new()
			.post("https://hitman-resources.netlify.app/smf-api/deploy")
			.send()
	);

	let mut config: Config = from_slice(&fs::read("config.json").wrap_err("Couldn't read config.json")?)
		.wrap_err("Couldn't parse config.json")?;

	let Some(game_path) = config.game_path.to_owned() else {
		intentional_halt!("No game path has been selected. Please select a game and try again.");
	};

	if !game_path.join("thumbs.dat").exists() {
		intentional_halt!(
			"Can't find thumbs.dat in the game path! The framework may be misconfigured, or in the wrong folder."
		);
	}

	let (hash_list_result, world_result) = rayon::join(
		|| {
			let _span = tracing::info_span!("Loading hash list").entered();
			HASH_LIST.load_cached().wrap_err("Couldn't load hash list")
		},
		|| {
			let ipc = ipc_name.map(IpcSender::connect).transpose()?;

			let mods = Filesystem::new("Mods")?;
			let diagnostics = LogDiagnostics;

			let Some(game) = (match detect_game(game_path) {
				Ok(x) => x,
				Err(e) => {
					if e.downcast_ref::<IntentionalHalt>().is_some()
						&& let Some(ipc) = &ipc
					{
						ipc.send(serde_brief::to_vec(&DeployMessage::UnrecognisedGameVersion)?)?;
					}

					return Err(e);
				}
			}) else {
				intentional_halt!(
					"Couldn't find any game executable in the game path! The framework may be misconfigured, or in \
					 the wrong folder."
				);
			};

			log::info!("Deploying mods to {} install at {}", game, game.path.to_string_lossy());

			let game = GameContext::from_game(game, &diagnostics, config.use_alternative_output_directory.is_none())?;

			if sort_deploy_order(&mut config, &mods, (&game).into())? {
				fs::write("config.json", format_json(&serde_json::to_string(&config)?)?)
					.wrap_err("Couldn't write new config")?;
			}

			let world = Arc::new(IPCWrapper {
				world: DeployWorld {
					mods,
					output: if let Some(custom_output) = &config.use_alternative_output_directory {
						GameOutput::from_custom(&game, custom_output)
					} else {
						GameOutput::from_game(&game)
					},
					diagnostics,
					progress
				},
				ipc
			});

			let game = Arc::new(game);

			color_eyre::eyre::Ok((game, world))
		}
	);

	hash_list_result?;
	let (game, world) = world_result?;

	let config = Arc::new(config);

	log::info!("Loading cache");

	let cache = Cache::new("cache", &game.hash)?;
	let (mut deploy_graph, mut peacock_plugins, mut sdk_mods, mut resources_to_port) =
		analyse(&config, &world, &game, &cache)?;

	migrate(
		&config,
		&world,
		&game,
		&cache,
		&mut deploy_graph,
		&mut peacock_plugins,
		&mut sdk_mods,
		&mut resources_to_port
	)?;

	cache.gc(&deploy_graph);

	let mut state = {
		let _span = tracing::info_span!("Initialising state").entered();

		State {
			deployment: DeployContext::new(&config, &game)?,
			localisation_hash_list: tonytools::hashlist::HashList::load(
				&fs::read("tonytools_hash_list.hmla").wrap_err("Couldn't read TonyTools hash list")?
			)
			.wrap_err("Couldn't load TonyTools hash list")?
			.into(),
			game,
			config,
			world
		}
	};

	state
		.prepare(&deploy_graph, peacock_plugins, sdk_mods, resources_to_port, &cache)
		.await?;

	(cache, state, deploy_graph)
}

#[try_fn]
#[instrument(skip_all)]
pub async fn deploy(progress: Arc<dyn Progress + Send + Sync>) -> Result<()> {
	let start_time = Instant::now();

	let (cache, state, deploy_graph) = initialise(progress, Arguments::from_env().opt_value_from_str("--ipc")?).await?;

	let cache = Arc::new(cache);
	let state = Arc::new(state);

	log::info!("Evaluating deploy graph");
	deploy_graph.evaluate(&cache, &state).await?;

	let cache = Arc::into_inner(cache).unwrap();
	let state = Arc::into_inner(state).unwrap();

	log::info!("Cleaning up cache");
	cache.persist()?;

	let (config, game, world, server_side_data, server_side_assets) = state.finish().await?;

	let mod_versions = config
		.deploy_order
		.iter()
		.map(|mod_id| Ok((mod_id.to_owned(), world.get_mod_manifest(mod_id)?.version.to_owned())))
		.collect::<Result<HashMap<_, _>>>()?;

	log::info!("Finalising");

	let smf_appdata_dir = dirs::data_local_dir()
		.ok_or_eyre("No app data directory found")?
		.join("Simple Mod Framework");

	let deployment_dir = smf_appdata_dir.join("deployments").join(format!(
		"{:x}",
		md5::compute(fs::canonicalize(&game.retail_path)?.to_string_lossy().deref())
	));
	let _ = fs::remove_dir_all(&deployment_dir);
	fs::create_dir_all(&deployment_dir).wrap_err("Couldn't create deploy summary directory")?;

	fs::write(
		deployment_dir.join("summary.json"),
		serde_json::to_vec(&DeploySummary {
			config: (*config).to_owned(),
			mod_versions: mod_versions.to_owned(),
			game: Game {
				version: game.version,
				platform: game.platform,
				hash: game.hash.to_owned(),
				path: game.retail_path.to_owned()
			},
			framework_version: SemVer(APP_VERSION.to_owned()),
			framework_path: current_dir()?,
			server_side_data,
			total_time: start_time.elapsed().as_millis().try_into()?
		})?
	)?;

	for (asset_id, data) in server_side_assets {
		fs::write(deployment_dir.join(asset_id.to_string()), data)?;
	}

	if config.online_services {
		// Submit analytics (errors are ignored because it doesn't matter too much)
		let _ = AnalyticsSubmission {
			config: AnalyticsConfig {
				skip_intro: config.skip_intro,
				use_alternative_output_directory: config.use_alternative_output_directory.is_some(),
				mod_options: config
					.mod_options
					.clone()
					.into_iter()
					.filter(|(x, _)| config.deploy_order.contains(x))
					.collect(),
				deploy_order: config.deploy_order.to_owned(),
				developer_mode: config.developer_mode,
				ui_locale: config.ui_locale.to_owned(),
				auto_disable_dynres: config.auto_disable_dynres,
				boot_scene: config.boot_scene.is_some()
			},
			mod_versions,
			version: SemVer((*APP_VERSION).to_owned()),
			experiment: EXPERIMENT.to_owned().map(|x| x.into()),
			platform: VersionPlatform {
				version: game.version,
				platform: game.platform
			},
			game_hash: game.hash.to_owned(),
			result: AnalyticsDeployResult::Successful {
				total_time: start_time.elapsed().as_millis().try_into()?
			}
		}
		.submit()
		.await;
	}

	log::info!("Done in {}", hrtime::from_sec_padded(start_time.elapsed().as_secs()));
}

#[try_fn]
pub async fn main() -> Result<ExitCode, color_eyre::Report> {
	let _guard = if Arguments::from_env().contains("--profile") {
		let (chrome_layer, _guard) = ChromeLayerBuilder::new()
			.trace_style(TraceStyle::Async)
			.include_args(true)
			.include_locations(false)
			.file(
				dirs::data_local_dir()
					.ok_or_eyre("No app data directory found")?
					.join("Simple Mod Framework")
					.join("tracing.json")
			)
			.build();

		tracing_subscriber::registry()
			.with(ErrorLayer::default())
			.with(chrome_layer)
			.init();

		Some(_guard)
	} else {
		tracing_subscriber::registry().with(ErrorLayer::default()).init();
		None
	};

	color_eyre::config::HookBuilder::new()
		.panic_message(PanicReporter)
		.display_env_section(false)
		.install()?;

	if Path::new("Deploy.log").exists() {
		fs::remove_file("Deploy.log")?;
	}

	let line_colors = ColoredLevelConfig::new()
		.error(Color::White)
		.warn(Color::White)
		.info(Color::White)
		.debug(Color::BrightBlack)
		.trace(Color::BrightBlack);

	let level_colors = line_colors.error(Color::Red).warn(Color::Yellow).info(Color::Blue);

	let logger = fern::Dispatch::new()
		.chain(
			fern::Dispatch::new()
				.format(move |out, message, record| {
					out.finish(format_args!(
						"{level_color}{level}\x1B[0m\t{target_color}{target}\x1B[0m{line_color}\t{message}\x1B[0m",
						line_color = format_args!("\x1B[{}m", line_colors.get_color(&record.level()).to_fg_str()),
						level_color = format_args!("\x1B[{}m", level_colors.get_color(&record.level()).to_fg_str()),
						level = match record.level() {
							log::Level::Trace => "Trace",
							log::Level::Debug => "Detail",
							log::Level::Info => "Info",
							log::Level::Warn => "Warning",
							log::Level::Error => "Error"
						},
						target_color = format_args!(
							"\x1B[{}m",
							match line_colors.get_color(&record.level()) {
								Color::BrightBlack => Color::BrightBlack,
								_ => Color::Magenta
							}
							.to_fg_str()
						),
						target = if record.target().starts_with("simple_mod_framework") {
							"Main"
						} else {
							record.target()
						},
						message = message
					))
				})
				.level(log::LevelFilter::Debug)
				.chain(std::io::stdout())
		)
		.chain(
			fern::Dispatch::new()
				.format(|out, message, record| {
					out.finish(format_args!(
						"{}\t{}\t{}\t{}:{}\t{}",
						SystemTime::now()
							.duration_since(UNIX_EPOCH)
							.expect("Clock went backwards?")
							.as_millis(),
						record.level(),
						record.target(),
						record.file().expect("Couldn't get file for log message!"),
						record.line().expect("Couldn't get line for log message!"),
						message
					))
				})
				.chain(fern::log_file("Deploy.log")?)
		)
		.level_for("reqwest", log::LevelFilter::Off)
		.level_for("rustls_platform_verifier", log::LevelFilter::Off)
		.level_for("fjall", log::LevelFilter::Error)
		.level_for("lsm_tree", log::LevelFilter::Error)
		.level_for("sfa", log::LevelFilter::Error);

	let indicatif = (std::io::stdin().is_terminal() && !Arguments::from_env().contains("--non-interactive"))
		.then(MultiProgress::new);

	let progress = if let Some(indicatif) = &indicatif {
		let (level, logger) = logger.into_log();
		LogWrapper::new(indicatif.clone(), logger).try_init()?;
		log::set_max_level(level);
		Arc::new(CliProgress::new(indicatif.clone())) as _
	} else {
		logger.apply()?;
		Arc::new(LogProgress) as _
	};

	match deploy(progress).await {
		Ok(_) => ExitCode::SUCCESS,
		Err(e) => {
			if let Some(indicatif) = indicatif {
				indicatif.clear()?;
			}

			let config: Config = from_slice(&fs::read("config.json").wrap_err("Couldn't read config.json file")?)
				.wrap_err("Couldn't parse config.json file")?;

			if let Some(err) = e.downcast_ref::<IntentionalHalt>() {
				let _: Result<_> = try {
					if config.online_services
						&& let Some(game) = detect_game(config.game_path.to_owned().ok_or_eyre("No game selected")?)?
					{
						let _ = (AnalyticsSubmission {
							config: AnalyticsConfig {
								skip_intro: config.skip_intro,
								use_alternative_output_directory: config.use_alternative_output_directory.is_some(),
								mod_options: config
									.mod_options
									.into_iter()
									.filter(|(x, _)| config.deploy_order.contains(x))
									.collect(),
								deploy_order: config.deploy_order.to_owned(),
								developer_mode: config.developer_mode,
								ui_locale: config.ui_locale.to_owned(),
								auto_disable_dynres: config.auto_disable_dynres,
								boot_scene: config.boot_scene.is_some()
							},
							version: SemVer((*APP_VERSION).to_owned()),
							experiment: EXPERIMENT.to_owned().map(|x| x.into()),
							platform: (&game).into(),
							game_hash: game.hash,
							mod_versions: config
								.deploy_order
								.iter()
								.map(|mod_id| {
									Ok((
										mod_id.to_owned(),
										Filesystem::new("Mods")?.get_mod_manifest(mod_id)?.version.to_owned()
									))
								})
								.collect::<Result<HashMap<_, _>>>()?,
							result: AnalyticsDeployResult::Unsuccessful {
								failure_reason: DeployUnsuccessfulReason::IntentionalHalt,
								errors: vec![err.to_string()]
							}
						})
						.submit()
						.await;
					}
				};

				if let Some(target) = &err.target {
					log::error!(target: &target, "{}", err.message);

					eprintln!();
					eprintln!("{}", "Deployment has been prevented due to a mod problem.".red());
					eprintln!(
						"The original error is shown above, as well as the mod which caused it. Removing the mod may \
						 fix this issue."
					);
				} else {
					log::error!("{}", err.message);

					eprintln!();
					eprintln!("{}", "Deployment has been prevented due to a detected issue.".red());

					if err.message.contains("Unrecognised game version") {
						eprintln!(
							"Do not post a comment about this. If the game recently updated, wait for a Simple Mod \
							 Framework update - the developers are already aware."
						);
					} else {
						eprintln!("Fix the issue shown above and try again.");
					}
				}
			} else {
				let game = detect_game(config.game_path.to_owned().ok_or_eyre("No game selected")?)?;

				if let Some(game) = game.as_ref()
					&& config.online_services
				{
					let _ = (AnalyticsSubmission {
						config: AnalyticsConfig {
							skip_intro: config.skip_intro,
							use_alternative_output_directory: config.use_alternative_output_directory.is_some(),
							mod_options: config
								.mod_options
								.into_iter()
								.filter(|(x, _)| config.deploy_order.contains(x))
								.collect(),
							deploy_order: config.deploy_order.to_owned(),
							developer_mode: config.developer_mode,
							ui_locale: config.ui_locale.to_owned(),
							auto_disable_dynres: config.auto_disable_dynres,
							boot_scene: config.boot_scene.is_some()
						},
						version: SemVer((*APP_VERSION).to_owned()),
						experiment: EXPERIMENT.to_owned().map(|x| x.into()),
						game_hash: game.hash.to_owned(),
						platform: game.into(),
						mod_versions: config
							.deploy_order
							.iter()
							.map(|mod_id| {
								Ok((
									mod_id.to_owned(),
									Filesystem::new("Mods")?.get_mod_manifest(mod_id)?.version.to_owned()
								))
							})
							.collect::<Result<HashMap<_, _>>>()?,
						result: AnalyticsDeployResult::Unsuccessful {
							failure_reason: if format!("{e:?}").contains("this error is due to a mod problem") {
								DeployUnsuccessfulReason::IntentionalError
							} else {
								DeployUnsuccessfulReason::UnintentionalError
							},
							errors: vec![regex_replace_all!(r"\x1b\[[0-9;]*m", format!("{e:?}").trim(), "").into()]
						}
					})
					.submit()
					.await;
				}

				let mut profile = File::create(DEBUG_PROFILE_DIR.join("error.txt"))?;
				writeln!(profile, "Deployment failed due to an error.")?;
				writeln!(
					profile,
					"The game version was: {}, {}",
					game.as_ref()
						.map(|x| VersionPlatform::from(x).to_string())
						.unwrap_or_else(|| "unavailable".into()),
					game.as_ref().map(|x| x.hash.deref()).unwrap_or("<unavailable>")
				)?;
				writeln!(
					profile,
					"\nError:\n   {}",
					regex_replace_all!(r"\x1b\[[0-9;]*m", format!("{e:?}").trim(), "")
				)?;

				let err_to_display = format!("{:?}", {
					if config.developer_mode {
						e.note("see the created `debug` folder for more information")
					} else {
						e
					}
				});

				let err_to_display = regex_replace_all!(r"^ *Location:\r?\n.*\r?\n\r?\n"m, err_to_display.trim(), "");
				let err_to_display = regex_replace_all!(
					r"^(\s|\u2501)+SPANTRACE(\s|\u2501)+\r?\n\r?\n[\s\S]+?\r?\n\r?\n"m,
					&err_to_display,
					""
				);
				let err_to_display = regex_replace_all!(
					r"^( *)(\d+): "m,
					&err_to_display,
					|_, spaces: &str, number: &str| format!("{}{}. ", spaces, number.parse::<usize>().unwrap() + 1)
				);

				if err_to_display.contains("this error is due to a mod problem") {
					eprintln!();
					eprintln!("{}", "Deployment has failed due to an error.".red());
					eprintln!("The error is shown below.");
					eprintln!();
				} else {
					eprintln!();
					eprintln!("{}", "Deployment has failed due to an error.".red());
					eprintln!("This could be caused by a mod, by something you've done, or by SMF itself.");
					eprintln!("If you can't fix this, and you think it's a bug with SMF, you may want to report it at https://hitman-resources.netlify.app/issue-report");
					eprintln!();
				}

				eprintln!("Error:\n   {err_to_display}");
			}

			ExitCode::FAILURE
		}
	}
}
