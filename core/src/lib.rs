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
		"e72ee951f153bd8858f37e36d09cfaf5": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // base game
		"a8761edd766e4016a9ccc64a8c5ed883": VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic }, // ansel unlock
		"ceaff768215bf97014f023673f65c4c8": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // base game
		"fa2caabd0293edab43d44e823c1f152e": VersionPlatform { version: GlacierGame::H3, platform: Platform::Steam }, // ansel unlock

		// Gamepass/store protects the EXE from reading so we can't hash it, instead we hash the game config
		"a1d3d1698bc416014aad875c36957502": VersionPlatform { version: GlacierGame::H3, platform: Platform::Microsoft },

		"81b184536184d61301b57c47f492101f": VersionPlatform { version: GlacierGame::FL, platform: Platform::Steam },
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
