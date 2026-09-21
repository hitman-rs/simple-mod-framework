#![allow(non_snake_case)]

mod commands;

use std::{
	fs,
	path::{Path, PathBuf},
	process::ExitCode
};

use clap::Parser;
use color_eyre::eyre::{OptionExt, Result, WrapErr, bail};
use commands::{
	DeployError, DeployExitCode, DeployIPCMessage, DeployOutput, ModUpgradeProgress, rs_canonicalize_path,
	rs_copy_folder, rs_deploy, rs_detect_game, rs_detect_game_installs, rs_evaluate_condition, rs_extract_archive,
	rs_extract_rpkg, rs_get_experiment, rs_get_framework_version, rs_get_github_changelog, rs_get_github_download_url,
	rs_get_github_version, rs_get_installed_sdk_version, rs_get_latest_sdk_artifact, rs_get_latest_sdk_download_url,
	rs_get_latest_sdk_release, rs_get_latest_smf_version, rs_get_manifest_schema, rs_get_modworkshop_download_url,
	rs_get_modworkshop_version, rs_get_nexus_version, rs_get_proxied_mod_version, rs_get_smf_changelog,
	rs_get_tonytools_hash_list_version, rs_resolve_mod_relative_path, rs_save_debug_profile, rs_update_framework,
	rs_upgrade_mod, rs_validate_mod_folder, rs_validate_number_option, rs_validate_string_option
};
use fn_wrap_err::wrap_err;
use glacier_base::encryption::xtea::XteaConfig;
use glacier_commons::{game::GlacierGame, hash_list::HASH_LIST};
use glacier_ini::IniFileSystem;
use rpkg_rs::resource::{partition_manager::PartitionManager, pdefs::PackageDefinitionSource};
use serde::{Deserialize, Serialize};
use serde_json::json;
use simple_mod_framework::APP_VERSION;
use simple_mod_framework_types::{Config, DeploySummary, Manifest};
use specta::Type;
use tauri::Url;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_specta::{ErrorHandlingMode, Event};
use tryvial::try_fn;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(not(debug_assertions), windows))]
fn remove_windows_console() {
	unsafe {
		windows_sys::Win32::System::Console::FreeConsole();
		windows_sys::Win32::System::Console::SetStdHandle(
			windows_sys::Win32::System::Console::STD_INPUT_HANDLE,
			std::ptr::null_mut()
		);
		windows_sys::Win32::System::Console::SetStdHandle(
			windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE,
			std::ptr::null_mut()
		);
		windows_sys::Win32::System::Console::SetStdHandle(
			windows_sys::Win32::System::Console::STD_ERROR_HANDLE,
			std::ptr::null_mut()
		);
	}
}

#[derive(clap::Parser, Debug)]
#[command()]
struct Args {
	#[command(subcommand)]
	subcommand: Option<SMFCommand>
}

#[derive(clap::Subcommand, Debug)]
enum SMFCommand {
	Deploy {
		/// Whether to produce a Perfetto trace for the deployment process in SMF's app data directory.
		#[arg(long, default_value_t = false)]
		profile: bool,

		/// Whether to run the deployment process as if not in a terminal (removing elements like progress bars).
		#[arg(long, default_value_t = false)]
		non_interactive: bool,

		/// The name of an IPC channel to send rich diagnostic information through.
		#[arg(long, hide = true)]
		ipc: Option<String>,

		/// Whether to emit raw resource files next to the framework executable as well as RPKGs.
		#[arg(long, default_value_t = false)]
		emit_unpacked: bool
	},
	UpgradeMod {
		game_path: PathBuf,
		mod_folder: PathBuf
	},
	ValidateMod {
		mod_folder: PathBuf
	},
	GetSchema {
		of: String
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct SchemeRequestReceived(Url);

#[try_fn]
#[wrap_err("Couldn't load game files")]
pub fn load_h3_game_files(game_path: &Path) -> Result<PartitionManager> {
	let thumbs =
		IniFileSystem::from_path(game_path.join("thumbs.dat"), XteaConfig::Woa).wrap_err("Couldn't load thumbs.dat")?;

	let (Ok(proj_path), Ok(relative_runtime_path)) = (
		thumbs.option(("application", "PROJECT_PATH")),
		thumbs.option(("application", "RUNTIME_PATH"))
	) else {
		bail!("thumbs.dat was missing required properties");
	};

	// Workaround for the Linux filesystem's case sensitivity. Copied from GlacierKit (credit: Dafitius)
	let relative_runtime_path_uppercased = relative_runtime_path
		.char_indices()
		.map(|(idx, ch)| if idx == 0 { ch.to_ascii_uppercase() } else { ch })
		.collect::<String>();

	let runtime_folder = [relative_runtime_path, relative_runtime_path_uppercased]
		.iter()
		.flat_map(|folder| game_path.join(proj_path.replace('\\', "/")).join(folder).canonicalize())
		.find(|joined_path| joined_path.exists())
		.ok_or_eyre("Couldn't find valid runtime folder")?;

	let mut partitions = PackageDefinitionSource::HM3(fs::read(runtime_folder.join("packagedefinition.txt"))?)
		.read()
		.wrap_err("Couldn't read packagedefinition")?;

	for partition in &mut partitions {
		partition.set_max_patch_level(9);
	}

	let mut partition_manager = PartitionManager::new(
		runtime_folder,
		GlacierGame::H3.into(),
		&PackageDefinitionSource::Custom(partitions)
	)
	.wrap_err("Couldn't create partition manager")?;

	partition_manager
		.mount_partitions(|_, _| {})
		.wrap_err("Couldn't mount partitions")?;

	partition_manager
}

fn main() -> ExitCode {
	std::env::set_current_dir({
		let mut x = std::env::current_exe().unwrap();
		x.pop();
		x
	})
	.unwrap();

	if !Path::new("Mods").exists() {
		fs::create_dir("Mods").unwrap();

		fs::write(
			Path::new("Mods").join("DO NOT TOUCH THIS FOLDER"),
			"This folder is managed by SMF. Copying files/folders into it can break the framework. Do not modify this \
			 folder unless you know what you are doing."
		)
		.unwrap();
	}

	if !Path::new("config.json").exists()
		|| serde_json::from_slice::<Config>(&fs::read("config.json").unwrap()).is_err()
	{
		fs::write("config.json", serde_json::to_string(&Config::default()).unwrap()).unwrap();
	}

	if Path::new("Simple Mod Framework-old.exe").exists() {
		fs::remove_file("Simple Mod Framework-old.exe").unwrap();
	}

	if Path::new("./Simple Mod Framework-old").exists() {
		fs::remove_file("./Simple Mod Framework-old").unwrap();
	}

	match Args::parse().subcommand {
		Some(SMFCommand::Deploy { .. }) => {
			return tauri::async_runtime::block_on(simple_mod_framework::run::main()).unwrap();
		}

		Some(SMFCommand::UpgradeMod { game_path, mod_folder }) => {
			tauri::async_runtime::block_on(HASH_LIST.load_latest()).unwrap();
			simple_mod_framework_upgrade::upgrade_mod(
				&load_h3_game_files(&game_path).unwrap(),
				&tonytools::hashlist::HashList::load(&fs::read("tonytools_hash_list.hmla").unwrap()).unwrap(),
				&reqwest::blocking::Client::new()
					.get("https://hitman-resources.netlify.app/smf-api/v2-mods")
					.send()
					.unwrap()
					.json()
					.unwrap(),
				mod_folder,
				APP_VERSION.to_owned(),
				|progress| {
					println!("{progress}");
				}
			)
			.unwrap();
		}

		Some(SMFCommand::ValidateMod { mod_folder }) => {
			tauri::async_runtime::block_on(HASH_LIST.load_latest()).unwrap();
			println!(
				"{}",
				match simple_mod_framework::validation::validate_mod_folder(mod_folder, false) {
					Ok(x) => serde_json::to_string(&x).unwrap(),
					Err(e) => serde_json::to_string(&json!({ "result": "error", "message": e.to_string() })).unwrap()
				}
			);
		}

		Some(SMFCommand::GetSchema { of }) => match of.as_ref() {
			"manifest" => println!(
				"{}",
				serde_json::to_string(&schemars::schema_for!(simple_mod_framework_types::Manifest)).unwrap()
			),

			_ => panic!("Unknown schema type")
		},

		None => {
			#[cfg(all(not(debug_assertions), windows))]
			remove_windows_console();

			tauri::async_runtime::spawn(HASH_LIST.load_latest());

			let specta = tauri_specta::Builder::<tauri::Wry>::new()
				.error_handling(ErrorHandlingMode::Throw)
				.typ::<Manifest>()
				.typ::<DeploySummary>()
				.commands(tauri_specta::collect_commands![
					rs_get_framework_version,
					rs_get_latest_smf_version,
					rs_get_smf_changelog,
					rs_get_nexus_version,
					rs_get_github_version,
					rs_get_github_download_url,
					rs_get_github_changelog,
					rs_get_modworkshop_version,
					rs_get_modworkshop_download_url,
					rs_extract_archive,
					rs_update_framework,
					rs_resolve_mod_relative_path,
					rs_evaluate_condition,
					rs_validate_mod_folder,
					rs_copy_folder,
					rs_get_manifest_schema,
					rs_get_experiment,
					rs_get_proxied_mod_version,
					rs_validate_number_option,
					rs_extract_rpkg,
					rs_save_debug_profile,
					rs_get_tonytools_hash_list_version,
					rs_detect_game_installs,
					rs_deploy,
					rs_upgrade_mod,
					rs_get_latest_sdk_release,
					rs_get_latest_sdk_download_url,
					rs_get_installed_sdk_version,
					rs_detect_game,
					rs_validate_string_option,
					rs_canonicalize_path,
					rs_get_latest_sdk_artifact
				])
				.events(tauri_specta::collect_events![
					DeployIPCMessage,
					DeployError,
					DeployOutput,
					DeployExitCode,
					ModUpgradeProgress
				]);

			#[cfg(debug_assertions)]
			if Path::new("../../main/src/lib").is_dir() {
				specta
					.export(
						specta_typescript::Typescript::default().header("/* eslint-disable */"),
						"../../main/src/lib/bindings.ts"
					)
					.expect("Failed to export bindings");
			}

			tauri::Builder::default()
				.plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
				.plugin(tauri_plugin_deep_link::init())
				.plugin(tauri_plugin_clipboard_manager::init())
				.plugin(tauri_plugin_process::init())
				.plugin(tauri_plugin_fs::init())
				.plugin(tauri_plugin_shell::init())
				.plugin(tauri_plugin_os::init())
				.plugin(tauri_plugin_http::init())
				.plugin(tauri_plugin_dialog::init())
				.invoke_handler(specta.invoke_handler())
				.setup(move |app| {
					specta.mount_events(app);

					#[cfg(any(windows, target_os = "linux"))]
					{
						app.deep_link().register_all().unwrap();
					}

					let start_urls = app.deep_link().get_current().unwrap();
					if let Some(mut urls) = start_urls
						&& let Some(url) = urls.pop()
					{
						SchemeRequestReceived(url).emit(app).unwrap();
					}

					let handle = app.handle().clone();
					app.deep_link().on_open_url(move |event| {
						let mut urls = event.urls();
						if let Some(url) = urls.pop() {
							SchemeRequestReceived(url).emit(&handle).unwrap();
						}
					});

					Ok(())
				})
				.plugin(tauri_plugin_upload::init())
				.run(tauri::generate_context!())
				.expect("error while running tauri application");
		}
	}

	ExitCode::SUCCESS
}
