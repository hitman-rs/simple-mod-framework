use std::{fs, path::Path};

use color_eyre::eyre::{OptionExt, Result, WrapErr};
use fn_wrap_err::wrap_err;
use glacier_commons::game::GlacierGame;
use itertools::Itertools;
use pelite::{FileMap, PeFile};
use semver::Version;
use serde::Deserialize;
use tracing::instrument;
use tryvial::try_fn;
use url::Url;

#[derive(Deserialize)]
struct GithubArtifacts {
	artifacts: Vec<GithubArtifact>
}

#[derive(Deserialize)]
struct GithubArtifact {
	id: u64,
	name: String,
	workflow_run: GithubWorkflowRun
}

#[derive(Deserialize)]
struct GithubWorkflowRun {
	head_branch: String,
	head_sha: String
}

#[derive(Deserialize)]
struct GithubRelease {
	assets: Vec<GithubReleaseAsset>,
	tag_name: String
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
	browser_download_url: Url
}

fn repo_for(version: GlacierGame) -> Option<&'static str> {
	match version {
		GlacierGame::FL => Some("OrfeasZ/ZKntSDK"),
		GlacierGame::H3 => Some("OrfeasZ/ZHMModSDK"),
		_ => None
	}
}

#[try_fn]
#[wrap_err("Couldn't get latest SDK release")]
#[instrument]
pub async fn get_latest_sdk_release(version: GlacierGame) -> Result<Version> {
	let release = reqwest::Client::new()
		.get(format!(
			"https://api.github.com/repos/{}/releases/latest",
			repo_for(version).ok_or_eyre("Unsupported game version")?
		))
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2026-03-10")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<GithubRelease>()
		.await
		.wrap_err("GitHub API response was invalid")?;

	release.tag_name[1..]
		.parse()
		.wrap_err("Latest SDK version is not valid semver")?
}

#[try_fn]
#[wrap_err("Couldn't get latest SDK artifact")]
#[instrument]
pub async fn get_latest_sdk_artifact(version: GlacierGame) -> Result<String> {
	let response = reqwest::Client::new()
		.get(format!(
			"https://api.github.com/repos/{}/actions/artifacts",
			repo_for(version).ok_or_eyre("Unsupported game version")?
		))
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2026-03-10")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<GithubArtifacts>()
		.await
		.wrap_err("GitHub API response was invalid")?;

	response
		.artifacts
		.into_iter()
		.find(|x| x.name.ends_with("-Release") && x.workflow_run.head_branch == "master")
		.ok_or_eyre("No valid artifact found")?
		.workflow_run
		.head_sha
}

#[try_fn]
#[wrap_err("Couldn't get latest SDK download URL")]
#[instrument]
pub async fn get_latest_sdk_download_url(version: GlacierGame, artifact: bool) -> Result<Url> {
	if artifact {
		let response = reqwest::Client::new()
			.get(format!(
				"https://api.github.com/repos/{}/actions/artifacts",
				repo_for(version).ok_or_eyre("Unsupported game version")?
			))
			.header("User-Agent", "Simple Mod Framework")
			.header("Accept", "application/vnd.github+json")
			.header("X-GitHub-Api-Version", "2026-03-10")
			.send()
			.await
			.wrap_err("Failed to make web request to GitHub")?
			.json::<GithubArtifacts>()
			.await
			.wrap_err("GitHub API response was invalid")?;

		format!(
			"https://nightly.link/{}/actions/artifacts/{}.zip",
			repo_for(version).ok_or_eyre("Unsupported game version")?,
			response
				.artifacts
				.into_iter()
				.find(|x| x.name.ends_with("-Release") && x.workflow_run.head_branch == "master")
				.ok_or_eyre("No valid artifact found")?
				.id
		)
		.parse()?
	} else {
		let release = reqwest::Client::new()
			.get(format!(
				"https://api.github.com/repos/{}/releases/latest",
				repo_for(version).ok_or_eyre("Unsupported game version")?
			))
			.header("User-Agent", "Simple Mod Framework")
			.header("Accept", "application/vnd.github+json")
			.header("X-GitHub-Api-Version", "2026-03-10")
			.send()
			.await
			.wrap_err("Failed to make web request to GitHub")?
			.json::<GithubRelease>()
			.await
			.wrap_err("GitHub API response was invalid")?;

		release
			.assets
			.into_iter()
			.map(|x| x.browser_download_url)
			.find(|x| x.path().contains("-Release.zip"))
			.ok_or_eyre("No valid asset found")?
	}
}

#[try_fn]
#[wrap_err("Couldn't get installed SDK version")]
pub fn get_installed_sdk_version(retail_path: impl AsRef<Path>) -> Result<Option<String>> {
	let retail_path = retail_path.as_ref();

	if !retail_path.join("dinput8.dll").exists() {
		return Ok(None);
	}

	let dll_path = if retail_path.join("ZHMModSDK.dll").exists() {
		retail_path.join("ZHMModSDK.dll")
	} else if retail_path.join("ZKntSdk.dll").exists() {
		retail_path.join("ZKntSdk.dll")
	} else {
		return Ok(None);
	};

	if retail_path.join("sdk.txt").exists() {
		return Ok(Some(fs::read_to_string(retail_path.join("sdk.txt"))?));
	}

	let dll = FileMap::open(&dll_path).wrap_err("Couldn't open SDK DLL")?;
	let dll = PeFile::from_bytes(&dll).wrap_err("Couldn't parse SDK DLL")?;

	let Ok(vi) = dll.resources()?.version_info() else {
		// Likely old SDK build which we can treat as not installed
		return Ok(None);
	};

	let vi = vi.file_info();

	let version = vi
		.strings
		.get(
			vi.strings
				.keys()
				.next()
				.ok_or_eyre("No strings data in SDK DLL version info")?
		)
		.ok_or_eyre("No language found in SDK DLL version info")?
		.get("ProductVersion")
		.ok_or_eyre("No ProductVersion field in SDK DLL version info")?;

	Some(
		version
			.chars()
			.rev()
			.skip(2)
			.collect_vec()
			.into_iter()
			.rev()
			.collect::<String>()
	)
}
