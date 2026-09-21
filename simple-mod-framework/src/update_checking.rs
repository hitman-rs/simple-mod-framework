use color_eyre::eyre::{OptionExt, Result, WrapErr, bail};
use fn_wrap_err::wrap_err;
use indexmap::IndexMap;
use itertools::Itertools;
use semver::Version;
use serde::Deserialize;
use skyscraper::{html, xpath};
use tracing::instrument;
use tryvial::try_fn;
use url::Url;

use crate::APP_VERSION;

#[wrap_err("Couldn't get version of Nexus Mods mod {id}")]
#[instrument]
pub async fn get_nexus_version(game: &str, id: u16) -> Result<Version> {
	let response = reqwest::get(format!("https://www.nexusmods.com/{game}/mods/{id}"))
		.await
		.wrap_err("Failed to make web request to Nexus Mods")?
		.text()
		.await?;

	let document = html::parse(&response)?;

	for stat in xpath::parse("//div[@class='statitem']")?.apply(&document)? {
		let node = stat.extract_as_node();
		if node
			.children(&document)
			.first()
			.ok_or_eyre("Stat had no title")?
			.text(&document)
			.ok_or_eyre("Stat title had no text")?
			== "Version"
		{
			return Ok(Version::parse(
				&node
					.children(&document)
					.get(1)
					.ok_or_eyre("Version stat had no value")?
					.text(&document)
					.ok_or_eyre("Version stat had no text")?
			)?);
		}
	}

	bail!("Couldn't find Version stat in Nexus Mods page: {response}");
}

#[derive(Deserialize)]
struct ModWorkshopResponse {
	version: Version,
	download: ModWorkshopDownload
}

#[derive(Deserialize)]
struct ModWorkshopDownload {
	download_url: Url
}

#[try_fn]
#[wrap_err("Couldn't get version of ModWorkshop mod {id}")]
#[instrument]
pub async fn get_modworkshop_version(id: u32) -> Result<Version> {
	reqwest::get(format!("https://api.modworkshop.net/mods/{id}"))
		.await
		.wrap_err("Failed to make web request to ModWorkshop")?
		.json::<ModWorkshopResponse>()
		.await
		.wrap_err("ModWorkshop API response was invalid")?
		.version
}

#[try_fn]
#[wrap_err("Couldn't get download URL of ModWorkshop mod {id}")]
#[instrument]
pub async fn get_modworkshop_download_url(id: u32) -> Result<Url> {
	reqwest::get(format!("https://api.modworkshop.net/mods/{id}"))
		.await
		.wrap_err("Failed to make web request to ModWorkshop")?
		.json::<ModWorkshopResponse>()
		.await
		.wrap_err("ModWorkshop API response was invalid")?
		.download
		.download_url
}

#[derive(Deserialize)]
struct GithubRelease {
	assets: Vec<GithubReleaseAsset>,
	body: Option<String>,
	tag_name: Version
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
	browser_download_url: Url
}

#[try_fn]
#[wrap_err("Couldn't get latest SMF version")]
#[instrument]
pub async fn get_latest_smf_version() -> Result<Version> {
	reqwest::Client::new()
		.get("https://api.github.com/repos/hitman-rs/simple-mod-framework/releases/latest")
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<GithubRelease>()
		.await
		.wrap_err("GitHub API response was invalid")?
		.tag_name
}

#[try_fn]
#[wrap_err("Couldn't get SMF changelog")]
#[instrument]
pub async fn get_smf_changelog() -> Result<String> {
	let releases = reqwest::Client::new()
		.get("https://api.github.com/repos/hitman-rs/simple-mod-framework/releases")
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<Vec<GithubRelease>>()
		.await
		.wrap_err("GitHub API response was invalid")?;

	let changelogs = releases
		.into_iter()
		.filter(|x| x.tag_name > *APP_VERSION)
		.map(|x| x.body.unwrap_or_default())
		.collect_vec();

	process_changelogs(changelogs)
}

#[try_fn]
#[wrap_err("Couldn't get version of GitHub mod {repository}")]
#[instrument]
pub async fn get_github_version(repository: &str) -> Result<Version> {
	reqwest::Client::new()
		.get(format!("https://api.github.com/repos/{repository}/releases/latest"))
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<GithubRelease>()
		.await
		.wrap_err("GitHub API response was invalid")?
		.tag_name
}

#[wrap_err("Couldn't get download URL of GitHub mod {repository}")]
#[instrument]
pub async fn get_github_download_url(repository: &str) -> Result<Url> {
	for asset in reqwest::Client::new()
		.get(format!("https://api.github.com/repos/{repository}/releases/latest"))
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<GithubRelease>()
		.await
		.wrap_err("GitHub API response was invalid")?
		.assets
	{
		if asset.browser_download_url.path().ends_with(".zip") || asset.browser_download_url.path().ends_with(".7z") {
			return Ok(asset.browser_download_url);
		}
	}

	bail!("No ZIP asset was found on latest release");
}

#[try_fn]
#[wrap_err("Couldn't get changelog of GitHub mod {repository}")]
#[instrument]
pub async fn get_github_changelog(current_ver: &Version, repository: &str) -> Result<String> {
	let releases = reqwest::Client::new()
		.get(format!("https://api.github.com/repos/{repository}/releases"))
		.header("User-Agent", "Simple Mod Framework")
		.header("Accept", "application/vnd.github+json")
		.header("X-GitHub-Api-Version", "2022-11-28")
		.send()
		.await
		.wrap_err("Failed to make web request to GitHub")?
		.json::<Vec<GithubRelease>>()
		.await
		.wrap_err("GitHub API response was invalid")?;

	let changelogs = releases
		.into_iter()
		.filter(|x| x.tag_name > *current_ver)
		.map(|x| x.body.unwrap_or_default())
		.collect_vec();

	process_changelogs(changelogs)
}

fn process_changelogs(changelogs: Vec<String>) -> String {
	let mut sections: IndexMap<String, Vec<String>> = IndexMap::new();
	let mut current_section = sections.entry("".to_owned()).or_default();
	for changelog in changelogs {
		for line in changelog.lines() {
			let line = line.trim();

			if !line.is_empty() && !line.contains("hitman-resources.netlify.app/smf-install-link") {
				if line.starts_with("##") {
					current_section = sections.entry(line.to_owned()).or_default();
				} else {
					current_section.push(line.to_owned());
				}
			}
		}
	}

	sections
		.into_iter()
		.map(|(section, entries)| format!("{}\n{}", section, entries.join("\n")))
		.collect_vec()
		.join("\n")
}
