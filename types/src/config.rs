use std::{fmt::Debug, path::PathBuf};

use ecow::EcoString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uuid::Uuid;

use crate::{
	HashMap,
	common::{Game, ScriptError},
	manifest::{ModID, ModOptionID, SemVer}
};

/// The SMF config.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_functions(Self::r_new)]
#[rune(install_with = Self::rune_install)]
pub struct Config {
	pub game_path: Option<PathBuf>,

	#[rune(get, set)]
	pub skip_intro: bool,

	pub use_alternative_output_directory: Option<PathBuf>,

	pub deploy_order: Vec<ModID>,

	#[specta(type = std::collections::HashMap<ModID, std::collections::HashMap<ModOptionID, ModOptionValue>>)]
	pub mod_options: HashMap<ModID, HashMap<ModOptionID, ModOptionValue>>,

	#[rune(get, set)]
	pub developer_mode: bool,

	pub online_services: bool,

	pub auto_disable_dynres: bool,

	#[specta(type = std::collections::HashMap<Uuid, ModProfile>)]
	pub profiles: HashMap<Uuid, ModProfile>,

	#[rune(get, set)]
	pub ui_locale: String,

	#[rune(get, set)]
	pub boot_scene: Option<String>,

	/// GUI state irrelevant to the deploy process.
	#[specta(type = std::collections::HashMap<String, Value>)]
	pub gui: HashMap<String, Value>
}

impl Config {
	#[rune::function(path = Self::new)]
	fn r_new() -> Self {
		Self::default()
	}

	fn rune_install(module: &mut rune::Module) -> Result<(), rune::ContextError> {
		module.field_function(
			&rune::runtime::Protocol::GET,
			"deploy_order",
			|s: &Self| -> Vec<String> { s.deploy_order.iter().map(|x| x.to_string()).collect() }
		)?;

		module.field_function(
			&rune::runtime::Protocol::SET,
			"deploy_order",
			|s: &mut Self, value: Vec<String>| {
				s.deploy_order = value
					.into_iter()
					.map(EcoString::from)
					.map(ModID::try_from)
					.collect::<Result<_, _>>()
					.map_err(ScriptError::msg)
					.inspect_err(|e| log::error!("Failed to set deploy_order: {}", e))
					.unwrap_or_default();
			}
		)?;

		module.field_function(
			&rune::runtime::Protocol::GET,
			"mod_options",
			|s: &Self| -> std::collections::HashMap<String, std::collections::HashMap<String, ModOptionValue>> {
				s.mod_options
					.clone()
					.into_iter()
					.map(|(x, y)| (x.to_string(), y.into_iter().map(|(x, y)| (x.to_string(), y)).collect()))
					.collect()
			}
		)?;

		module.field_function(
			&rune::runtime::Protocol::SET,
			"mod_options",
			|s: &mut Self,
			 value: std::collections::HashMap<String, std::collections::HashMap<String, ModOptionValue>>| {
				s.mod_options = value
					.into_iter()
					.map(|(x, y)| {
						Ok::<_, ScriptError>((
							ModID::try_from(EcoString::from(x)).map_err(ScriptError::msg)?,
							y.into_iter()
								.map(|(x, y)| {
									Ok::<_, ScriptError>((
										ModOptionID::try_from(EcoString::from(x)).map_err(ScriptError::msg)?,
										y
									))
								})
								.collect::<Result<_, _>>()
								.map_err(ScriptError::msg)?
						))
					})
					.collect::<Result<_, _>>()
					.inspect_err(|e| log::error!("Failed to set mod_options: {}", e))
					.unwrap_or_default();
			}
		)?;

		Ok(())
	}
}

impl Default for Config {
	fn default() -> Self {
		Self {
			game_path: None,
			skip_intro: false,
			use_alternative_output_directory: None,
			deploy_order: vec![],
			mod_options: Default::default(),
			developer_mode: false,
			online_services: true,
			auto_disable_dynres: true,
			profiles: Default::default(),
			ui_locale: "en".to_string(),
			boot_scene: None,
			gui: Default::default()
		}
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type, JsonSchema, better_rune_derive::Any)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT)]
pub enum ModOptionValue {
	#[rune(constructor)]
	Boolean {
		#[rune(get, set)]
		value: bool
	},

	#[rune(constructor)]
	Number {
		#[rune(get, set)]
		value: f64
	},

	#[rune(constructor)]
	Selection {
		#[rune(get, set)]
		value: ModOptionID
	},

	#[rune(constructor)]
	Color {
		#[rune(get, set, as_into = String)]
		#[schemars(with = "String")]
		#[specta(type = String)]
		value: EcoString
	},

	#[rune(constructor)]
	String {
		#[rune(get, set, as_into = String)]
		#[schemars(with = "String")]
		#[specta(type = String)]
		value: EcoString
	}
}

impl ModOptionValue {
	pub fn get_value(&self) -> Value {
		match self {
			Self::Boolean { value } => (*value).into(),
			Self::Number { value } => (*value).into(),
			Self::Selection { value } => value.to_string().into(),
			Self::Color { value } => value.to_string().into(),
			Self::String { value } => value.to_string().into()
		}
	}
}

/// A mod profile.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModProfile {
	pub name: String,
	pub deploy_order: Vec<ModID>,

	#[specta(type = std::collections::HashMap<ModID, std::collections::HashMap<ModOptionID, ModOptionValue>>)]
	pub mod_options: HashMap<ModID, HashMap<ModOptionID, ModOptionValue>>
}

/// A summary of the deploy.
#[derive(Serialize, Deserialize, Type, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploySummary {
	pub game: Game,
	pub framework_path: PathBuf,
	pub framework_version: SemVer,
	pub config: Config,

	#[specta(type = std::collections::HashMap<ModID, SemVer>)]
	pub mod_versions: HashMap<ModID, SemVer>,

	pub server_side_data: ServerSideData,
	pub total_time: u32
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerSideData {
	pub unlockables: Option<Uuid>,
	pub repository: Option<Uuid>,

	#[specta(type = std::collections::HashMap<String, Uuid>)]
	pub contracts: HashMap<String, Uuid>,

	#[specta(type = std::collections::HashMap<String, Uuid>)]
	pub world_map_metadata: HashMap<String, Uuid>,

	pub story_config: Option<Uuid>,
	pub peacock_plugins: Vec<PathBuf>,
	pub dynamic_resources_disabled: bool
}
