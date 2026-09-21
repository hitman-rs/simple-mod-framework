mod common;
mod config;
mod manifest;

pub use common::*;
pub use config::*;
pub use manifest::*;

#[tryvial::try_fn]
pub fn rune_install(module: &mut rune::Module) -> color_eyre::Result<()> {
	module.ty::<Config>()?;
	module.ty::<Platform>()?;
	module.ty::<ScriptError>()?;
	module.ty::<ModOptionValue>()?;
	module.ty::<VersionPlatform>()?;
	module.ty::<ModID>()?;
	module.ty::<ModOptionID>()?;
	module.ty::<NonEmptyString>()?;
	module.ty::<SafeRelativePath>()?;
	module.ty::<ManifestData>()?;
	module.ty::<PackageDefinitionEntity>()?;
	module.ty::<Localisation>()?;
}

pub type HashMap<K, V, S = rapidhash::fast::RandomState> = std::collections::HashMap<K, V, S>;
pub type HashSet<K, S = rapidhash::fast::RandomState> = std::collections::HashSet<K, S>;
pub type PapayaMap<K, V, S = rapidhash::fast::RandomState> = papaya::HashMap<K, V, S>;
pub type PapayaSet<K, S = rapidhash::fast::RandomState> = papaya::HashSet<K, S>;
