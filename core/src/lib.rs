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

pub const H3_VERSION: &str = "3.300.0";

pub const FL_VERSION: &str = "1.3.2";

pub static GAME_HASHES: LazyLock<HashMap<&'static str, VersionPlatform>> = LazyLock::new(|| {
	velcro::map_iter! {
		"031999023e2d4d350316f083bc166dfa": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // base game
		"9f824d724165b2541b866c4abe6791b1": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // ansel unlock
		"93f969b75ed8406fc219a294ea21d0ac": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // base game
		"e703184c1d35c23cdde965c34fe42752": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // ansel unlock

		// Gamepass/store protects the EXE from reading so we can't hash it, instead we hash the game config
		"f746ca34f200d67711aac6530c43b4ff": VersionPlatform { version: GlacierGame::H3, platform: Platform::Microsoft },

		"f15c4c8df8d45798bb5afe866043cac8": VersionPlatform { version: GlacierGame::FL, platform: Platform::Steam },
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
