use std::sync::LazyLock;

use color_eyre::Result;
use glacier_commons::game::GlacierGame;
use itertools::Itertools;
use simple_mod_framework_types::{HashMap, Platform, VersionPlatform};
use tryvial::try_fn;

pub mod diagnostics;
pub mod game;
pub mod rkyv_helpers;
pub mod utils;
pub mod world;

pub const H3_VERSION: &str = "3.280.0";

pub const FL_VERSION: &str = "1.2.2";

pub static GAME_HASHES: LazyLock<HashMap<&'static str, VersionPlatform>> = LazyLock::new(|| {
	velcro::map_iter! {
		"9b85211686e6b95bfc5f48c961992a0b": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // base game
		"bf6b2e2c34bf76e4a9b7e23a3c519f9b": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // ansel unlock
		"9a17c533634f4b4bdbfa2e03cc72cfaf": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // base game
		"82484078c5a1a78e2f84e9f0ad67760f": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // ansel unlock

		// Gamepass/store protects the EXE from reading so we can't hash it, instead we hash the game config
		"7814483cb24ba31e9d3cc5d8c0977920": VersionPlatform { version: GlacierGame::H3, platform: Platform::Microsoft },

		"cb2718d0d8a9c2b1e13198eb92d3b013": VersionPlatform { version: GlacierGame::FL, platform: Platform::Steam },
	}
	.collect()
});

pub static CLEAN_PACKAGE_DEFINITION: LazyLock<String> = LazyLock::new(|| {
	include_str!("../assets/h3-packagedefinition.txt")
		.lines()
		.map(|x| x.trim())
		.collect_vec()
		.join("\r\n")
});

#[try_fn]
pub fn rune_install(module: &mut rune::Module) -> Result<()> {
	module.ty::<game::NominalPartition>()?;
	module.ty::<game::RealPartition>()?;
	module.ty::<game::ResourceSpecifier>()?;
}
