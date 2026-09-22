use std::{
	fs::{self, File},
	io::{BufRead, BufReader},
	path::{Path, PathBuf},
	process::Command
};

use base64::Engine;
use color_eyre::eyre::{OptionExt, Result, WrapErr, eyre};
use fs_extra::dir::CopyOptions;
use glacier_commons::{
	game::GlacierGame,
	game_detection::{GameInstall, detect_installs},
	metadata::ResourceMetadata
};
use ipc_channel::{IpcError, ipc::IpcOneShotServer};
use rpkg_rs::resource::{partition_manager::PartitionManager, resource_package::ResourcePackage};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};
use simple_mod_framework::{
	APP_VERSION, EXPERIMENT,
	run::DeployMessage,
	scripts::{ConditionContext, eval_condition},
	update_checking::{
		get_github_changelog, get_github_download_url, get_github_version, get_latest_smf_version,
		get_modworkshop_download_url, get_modworkshop_version, get_nexus_version, get_smf_changelog
	},
	validation::validate_mod_folder
};
use simple_mod_framework_core::game::detect_game;
use simple_mod_framework_types::{
	Config, Game, HashMap, Manifest, ModID, NumberOptionValidation, SafeRelativePath, SemVer, StringOptionValidation,
	ValidationResult, VersionPlatform
};
use simple_mod_framework_upgrade::{ModInfo, upgrade_mod};
use specta::Type;
use tauri::{AppHandle, Manager, async_runtime};
use tauri_specta::Event;
use tryvial::{try_block, try_fn};
use zip_archive::Archiver;

use crate::load_h3_game_files;

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct DeployIPCMessage(DeployMessage);

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct DeployError(String);

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct DeployOutput(String);

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct DeployExitCode(i32);

fn no_window(command: &mut Command) -> &mut Command {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
	}
	#[cfg(not(windows))]
	{
		command
	}
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub fn rs_deploy(app: AppHandle) -> Result<(), String> {
	let (ipc, ipc_name) = IpcOneShotServer::<Vec<u8>>::new().map_err(|x| format!("{x:?}"))?;

	let reader = duct::cmd!(
		std::env::current_exe().map_err(|x| format!("{x:?}"))?,
		"deploy",
		"--non-interactive",
		"--ipc",
		ipc_name
	)
	.before_spawn(|cmd| {
		no_window(cmd);
		Ok(())
	})
	.stderr_to_stdout()
	.unchecked()
	.reader()
	.map_err(|x| format!("{x:?}"))?;

	async_runtime::spawn_blocking({
		let app = app.clone();
		move || {
			let x: Result<_> = try_block! {
				let (rx, msg) = ipc.accept()?;

				DeployIPCMessage(serde_brief::from_slice(&msg)?).emit(&app)?;

				loop {
					match rx.recv() {
						Ok(msg) => DeployIPCMessage(serde_brief::from_slice(&msg)?).emit(&app)?,
						Err(IpcError::Disconnected) => break,
						Err(e) => Err(eyre!("IPC error: {e:?}"))?
					}
				}
			};

			if let Err(e) = x {
				DeployError(format!("{e:?}")).emit(&app).unwrap();
			}
		}
	});

	async_runtime::spawn_blocking(move || {
		let x: Result<_> = try_block! {
			for line in BufReader::new(&reader).lines() {
				DeployOutput(line?).emit(&app)?;
			}

			DeployExitCode(
				reader
					.try_wait()?
					.ok_or_eyre("No output")?
					.status
					.code()
					.ok_or_eyre("No exit code")?
			)
			.emit(&app)?;
		};

		if let Err(e) = x {
			DeployError(format!("{e:?}")).emit(&app).unwrap();
		}
	});
}

#[tauri::command]
#[specta::specta]
pub async fn rs_save_debug_profile() -> Result<(), String> {
	if let Some(handle) = rfd::AsyncFileDialog::new()
		.add_filter("ZIP files", &["zip"])
		.set_title("Save the ZIP file")
		.save_file()
		.await
	{
		let x: Result<_> = try_block! {
			let profile = Path::new("debug");

			fs::copy("Deploy.log", profile.join("Deploy.log"))?;
			fs::write(profile.join("system-info.txt"), os_info::get().to_string())?;

			fs::create_dir_all("archive")?;

			let mut archiver = Archiver::new();
			archiver.push(profile);
			archiver.set_destination("archive");
			archiver.archive().map_err(|x| eyre!("{x:?}"))?;

			fs::copy(
				Path::new("archive").join(format!("{}.zip", profile.file_name().unwrap().to_string_lossy())),
				handle.path()
			)?;

			fs::remove_dir_all("archive")?;
		};

		x.map_err(|x| format!("{:?}", x))
	} else {
		Ok(())
	}
}

#[tauri::command]
#[specta::specta]
pub fn rs_extract_rpkg(rpkg_path: PathBuf, to_folder: PathBuf) -> Result<(), String> {
	let x: Result<_> = try_block! {
		fs::create_dir_all(&to_folder)?;

		let rpkg = ResourcePackage::from_file(&rpkg_path, GlacierGame::H3.into()).wrap_err("Couldn't load RPKG")?;

		for (id, info) in rpkg.resources() {
			let data = rpkg
				.read_resource(id)
				.wrap_err_with(|| format!("Couldn't read resource {id}"))?;

			fs::write(
				to_folder.join(format!("{}.{}", id.to_hex_string(), info.data_type())),
				data
			)
			.wrap_err("Couldn't write extracted file")?;

			fs::write(
				to_folder.join(format!("{}.{}.metadata.json", id.to_hex_string(), info.data_type())),
				serde_json::to_vec(&ResourceMetadata::try_from(info)?)?
			)
			.wrap_err("Couldn't write extracted metadata JSON")?;
		}
	};

	x.map_err(|x| format!("{:?}", x))
}

#[tauri::command]
#[specta::specta]
pub async fn rs_get_tonytools_hash_list_version() -> Option<u32> {
	Some(
		tonytools::hashlist::HashList::load(&fs::read("tonytools_hash_list.hmla").ok()?)
			.ok()?
			.version
	)
}

#[tauri::command]
#[specta::specta]
pub fn rs_get_framework_version() -> String {
	APP_VERSION.to_string()
}

#[tauri::command]
#[specta::specta]
pub fn rs_get_experiment() -> Option<String> {
	EXPERIMENT.to_owned().map(|x| x.to_owned())
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_latest_smf_version() -> Result<String, String> {
	get_latest_smf_version()
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn rs_get_smf_changelog() -> Result<String, String> {
	get_smf_changelog().await.map_err(|x| format!("{:?}", x))
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_nexus_version(game: &str, id: u16) -> Result<String, String> {
	get_nexus_version(game, id)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_github_version(repository: String) -> Result<String, String> {
	get_github_version(&repository)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_github_download_url(repository: String) -> Result<String, String> {
	get_github_download_url(&repository)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn rs_get_github_changelog(current_ver: SemVer, repository: String) -> Result<String, String> {
	get_github_changelog(&current_ver, &repository)
		.await
		.map_err(|x| format!("{:?}", x))
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_modworkshop_version(id: u32) -> Result<String, String> {
	get_modworkshop_version(id)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_modworkshop_download_url(id: u32) -> Result<String, String> {
	get_modworkshop_download_url(id)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn rs_extract_archive(archive: PathBuf, output: PathBuf) -> Result<(), String> {
	async_runtime::spawn_blocking(move || {
		compress_tools::uncompress_archive(
			File::open(archive).map_err(|x| format!("{}", x))?,
			&output,
			compress_tools::Ownership::Ignore
		)
		.map_err(|x| format!("{}", x))
	})
	.await
	.map_err(|x| format!("{x}"))?
}

#[tauri::command]
#[specta::specta]
#[try_fn]
pub async fn rs_update_framework() -> Result<(), String> {
	fs::remove_file("tempArchive").map_err(|x| format!("{x}"))?;

	#[cfg(windows)]
	fs::rename("Simple Mod Framework.exe", "Simple Mod Framework-old.exe").map_err(|x| format!("{x}"))?;

	#[cfg(not(windows))]
	fs::rename("./Simple Mod Framework", "./Simple Mod Framework-old").map_err(|x| format!("{x}"))?;

	fs_extra::dir::copy(
		"update",
		".",
		&CopyOptions {
			overwrite: true,
			content_only: true,
			..Default::default()
		}
	)
	.map_err(|x| format!("{x}"))?;

	fs::remove_dir_all("update").map_err(|x| format!("{x}"))?;
}

#[tauri::command]
#[specta::specta]
#[try_fn]
pub async fn rs_resolve_mod_relative_path(mod_id: String, path: SafeRelativePath) -> Result<String, String> {
	path.to_logical_path(Path::new("Mods").join(mod_id))
		.to_string_lossy()
		.into()
}

#[tauri::command]
#[specta::specta]
pub async fn rs_evaluate_condition(
	attribution: String,
	condition: String,
	enabled_mod_versions: std::collections::HashMap<ModID, SemVer>,
	platform: VersionPlatform,
	config: Config
) -> Result<bool, String> {
	eval_condition(
		&attribution,
		&condition,
		enabled_mod_versions
			.into_iter()
			.map(|(x, y)| (x, y.0))
			.collect::<HashMap<_, _>>()
			.into(),
		ConditionContext { platform, config }
	)
	.map_err(|x| format!("{:?}", x))
}

#[tauri::command]
#[specta::specta]
pub async fn rs_validate_mod_folder(folder: PathBuf, lenient: bool) -> ValidationResult {
	validate_mod_folder(folder, lenient)
		.unwrap_or_else(|e| ValidationResult::Fail(format!("Couldn't validate mod folder: {}", e)))
}

#[tauri::command]
#[specta::specta]
#[try_fn]
pub async fn rs_copy_folder(from: PathBuf, to: PathBuf) -> Result<(), String> {
	fs_extra::dir::copy(
		from,
		to,
		&CopyOptions {
			overwrite: true,
			content_only: true,
			..Default::default()
		}
	)
	.map_err(|x| x.to_string())?;
}

#[tauri::command]
#[specta::specta]
#[try_fn]
pub fn rs_get_manifest_schema() -> Result<Value, String> {
	to_value(schema_for!(Manifest)).map_err(|x| x.to_string())?
}

#[tauri::command]
#[specta::specta]
#[try_fn]
pub async fn rs_get_proxied_mod_version(mod_url: String) -> Result<String, String> {
	reqwest::Client::new()
		.get("https://hitman-resources.netlify.app/smf-api/get-mod-version")
		.query(&[("modURL", base64::prelude::BASE64_STANDARD.encode(mod_url))])
		.send()
		.await
		.map_err(|x| x.to_string())?
		.text()
		.await
		.map_err(|x| x.to_string())?
}

#[tauri::command]
#[specta::specta]
pub fn rs_validate_number_option(option: NumberOptionValidation, value: f64) -> ValidationResult {
	option.validate(value)
}

#[tauri::command]
#[specta::specta]
pub fn rs_validate_string_option(option: StringOptionValidation, value: String) -> ValidationResult {
	option.validate(&value)
}

#[tauri::command]
#[specta::specta]
pub async fn rs_detect_game(path: PathBuf) -> Result<Option<Game>, String> {
	detect_game(path).map_err(|x| format!("{:?}", x))
}

#[tauri::command]
#[specta::specta]
pub async fn rs_detect_game_installs() -> Result<Vec<GameInstall>, String> {
	detect_installs().map_err(|x| format!("{:?}", x))
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
pub struct ModUpgradeProgress(String);

#[tauri::command]
#[specta::specta]
pub async fn rs_upgrade_mod(
	app: AppHandle,
	game_path: PathBuf,
	mod_info: std::collections::HashMap<String, ModInfo>,
	mod_folder: PathBuf
) -> Result<(), String> {
	async_runtime::spawn_blocking(move || {
		let game_files = if let Some(partition_manager) = app.try_state::<PartitionManager>() {
			partition_manager
		} else {
			app.manage(load_h3_game_files(&game_path)?);

			app.state::<PartitionManager>()
		};

		let tthl = if let Some(tthl) = app.try_state::<tonytools::hashlist::HashList>() {
			tthl
		} else {
			let tthl = tonytools::hashlist::HashList::load(
				&fs::read("tonytools_hash_list.hmla").wrap_err("Couldn't read TonyTools hash list")?
			)
			.wrap_err("Couldn't load TonyTools hash list")?;

			app.manage(tthl);

			app.state::<tonytools::hashlist::HashList>()
		};

		upgrade_mod(
			&game_files,
			&tthl,
			&mod_info.into_iter().collect(),
			mod_folder,
			APP_VERSION.to_owned(),
			|progress| {
				ModUpgradeProgress(progress.into()).emit(&app).unwrap();
			}
		)
	})
	.await
	.map_err(|x| format!("{:?}", x))?
	.map_err(|x| format!("{:?}", x))
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_latest_sdk_release(version: GlacierGame) -> Result<String, String> {
	simple_mod_framework::sdk::get_latest_sdk_release(version)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_latest_sdk_artifact(version: GlacierGame) -> Result<String, String> {
	simple_mod_framework::sdk::get_latest_sdk_artifact(version)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_latest_sdk_download_url(version: GlacierGame, artifact: bool) -> Result<String, String> {
	simple_mod_framework::sdk::get_latest_sdk_download_url(version, artifact)
		.await
		.map_err(|x| format!("{:?}", x))?
		.to_string()
}

#[try_fn]
#[tauri::command]
#[specta::specta]
pub async fn rs_get_installed_sdk_version(retail_path: PathBuf) -> Result<Option<String>, String> {
	simple_mod_framework::sdk::get_installed_sdk_version(&retail_path)
		.map_err(|x| format!("{:?}", x))?
		.map(|x| x.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn rs_canonicalize_path(path: PathBuf) -> Result<PathBuf, String> {
	fs::canonicalize(&path).map_err(|x| x.to_string())
}
