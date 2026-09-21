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

pub static APP_VERSION: LazyLock<Version> =
	LazyLock::new(|| Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo crate version is invalid semver"));

pub static EXPERIMENT: Option<&'static str> = Some("SMFv3 closed beta");
