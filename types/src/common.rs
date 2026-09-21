use std::{
	fmt::{self, Debug, Display},
	path::PathBuf
};

use glacier_commons::game::GlacierGame;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(better_rune_derive::Any)]
#[rune_derive(DEBUG_FMT, DISPLAY_FMT)]
#[repr(transparent)]
pub struct ScriptError(pub color_eyre::Report);

impl<T> From<T> for ScriptError
where
	color_eyre::eyre::Report: From<T>
{
	fn from(value: T) -> Self {
		Self(color_eyre::Report::from(value))
	}
}

impl ScriptError {
	pub fn msg(msg: impl Display + Debug + Send + Sync + 'static) -> Self {
		Self(color_eyre::Report::msg(msg))
	}

	pub fn wrap_err(self, msg: impl Display + Send + Sync + 'static) -> Self {
		Self(self.0.wrap_err(msg))
	}
}

pub trait ScriptResultExt<T> {
	fn wrap_err(self, msg: impl Display + Send + Sync + 'static) -> Result<T, ScriptError>;
}

impl<T, E> ScriptResultExt<T> for Result<T, E>
where
	E: Into<ScriptError>
{
	fn wrap_err(self, msg: impl Display + Send + Sync + 'static) -> Result<T, ScriptError> {
		self.map_err(|e| e.into().wrap_err(msg))
	}
}

impl Debug for ScriptError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self.0)
	}
}

impl Display for ScriptError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self.0)
	}
}

#[derive(
	Serialize, Deserialize, JsonSchema, Debug, Type, PartialEq, Eq, Hash, Clone, Copy, better_rune_derive::Any,
)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE, PARTIAL_EQ, EQ)]
pub enum Platform {
	#[rune(constructor)]
	Steam,

	#[rune(constructor)]
	Epic,

	#[rune(constructor)]
	Microsoft,

	#[rune(constructor)]
	GOG
}

impl Display for Platform {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}
/// Information about a game being deployed to.
#[derive(Serialize, Deserialize, Type, PartialEq, Eq, Hash, Debug, Clone, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
pub struct Game {
	#[rune(get, set)]
	pub version: GlacierGame,

	#[rune(get, set)]
	pub platform: Platform,

	/// An MD5 hash of the game executable (or game config, if Microsoft).
	#[rune(get, set)]
	pub hash: String,

	/// The Retail folder.
	pub path: PathBuf
}

impl Display for Game {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{} ({})", self.version, self.platform)
	}
}
