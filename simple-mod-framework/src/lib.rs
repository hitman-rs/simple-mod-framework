#![feature(try_blocks)]

use std::sync::LazyLock;

use semver::Version;
pub mod analysis;
mod analytics;
pub mod cache;
pub mod deploy;
pub use simple_mod_framework_core::diagnostics;
pub mod graph;
pub mod scripts;
pub mod state;
pub mod topo_sort;
pub mod update_checking;
pub mod utils;
pub mod validation;
pub use simple_mod_framework_core::world;
pub mod sdk;

#[cfg(feature = "run")]
pub mod run;

mod built_info {
	include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub static APP_VERSION: LazyLock<Version> =
	LazyLock::new(|| Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo crate version is invalid semver"));

pub static EXPERIMENT: Option<&'static str> =
	if built_info::CI_PLATFORM.is_some() && option_env!("SMF_RELEASE_BUILD").is_some() {
		Some("SMFv3 open beta")
	} else if built_info::CI_PLATFORM.is_some() {
		Some(constcat::concat!(
			"artifact ",
			built_info::GIT_VERSION
				.or(built_info::GIT_COMMIT_HASH_SHORT)
				.unwrap_or("unknown")
		))
	} else {
		Some(constcat::concat!(
			"build ",
			built_info::GIT_VERSION
				.or(built_info::GIT_COMMIT_HASH_SHORT)
				.unwrap_or("unknown"),
			if built_info::GIT_DIRTY.unwrap_or(false) {
				"-dirty-"
			} else {
				"-"
			},
			built_info::TARGET,
			"-",
			built_info::PROFILE
		))
	};
