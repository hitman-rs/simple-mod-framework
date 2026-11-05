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

pub const H3_VERSION: &str = "3.290.1";

pub const FL_VERSION: &str = "1.3.2";

pub static GAME_HASHES: LazyLock<HashMap<&'static str, VersionPlatform>> = LazyLock::new(|| {
	velcro::map_iter! {
		"9eeae97c841b81e0077ba9e1d401ac94": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // base game
		"bae6182412109eabc94d335bb38f08f1": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // ansel unlock
		"a56f59157f2ac0cb0254164bb0676661": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // base game
		"53e2cfc62c0cdda9ba572665e3213389": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // ansel unlock

		// Gamepass/store protects the EXE from reading so we can't hash it, instead we hash the game config
		"a1d3d1698bc416014aad875c36957502": VersionPlatform { version: GlacierGame::H3, platform: Platform::Microsoft },

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
