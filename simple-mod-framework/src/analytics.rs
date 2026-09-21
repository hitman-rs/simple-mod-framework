use color_eyre::eyre::{Result, bail};
use fn_wrap_err::wrap_err;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;
use tryvial::try_fn;

use simple_mod_framework_types::{HashMap, ModID, ModOptionID, ModOptionValue, SemVer, VersionPlatform};

#[derive(Serialize, Deserialize, Debug, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsConfig {
	pub skip_intro: bool,

	pub use_alternative_output_directory: bool,

	pub deploy_order: Vec<ModID>,

	#[specta(type = std::collections::HashMap<ModID, std::collections::HashMap<ModOptionID, ModOptionValue>>)]
	pub mod_options: HashMap<ModID, HashMap<ModOptionID, ModOptionValue>>,

	pub developer_mode: bool,

	pub auto_disable_dynres: bool,

	pub ui_locale: String,

	pub boot_scene: bool
}

#[derive(Serialize, Deserialize, Debug, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSubmission {
	pub config: AnalyticsConfig,

	#[specta(type = std::collections::HashMap<ModID, SemVer>)]
	pub mod_versions: HashMap<ModID, SemVer>,

	pub version: SemVer,
	pub experiment: Option<String>,
	pub game_hash: String,
	pub platform: VersionPlatform,

	pub result: AnalyticsDeployResult
}

#[derive(Serialize, Deserialize, Debug, Type, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AnalyticsDeployResult {
	Successful {
		total_time: u32
	},
	Unsuccessful {
		failure_reason: DeployUnsuccessfulReason,
		errors: Vec<String>
	}
}

#[derive(Serialize, Deserialize, Debug, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DeployUnsuccessfulReason {
	Panic,
	UnintentionalError,
	IntentionalError,
	IntentionalHalt
}

impl AnalyticsSubmission {
	#[try_fn]
	#[wrap_err("Couldn't submit analytics information")]
	pub async fn submit(self) -> Result<()> {
		if reqwest::Client::new()
			.post("https://hitman-resources.netlify.app/smf-api/submit-deploy")
			.header("Content-Type", "application/json")
			.body(serde_json::to_string(&self)?)
			.send()
			.await?
			.status()
			.as_u16() != 200
		{
			bail!("Unsuccessful submission")
		}
	}
}
