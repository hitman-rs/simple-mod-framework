//! Abstractions over the environment the framework is running in.
//! World traits provide access to mod contents and a way to emit output files and diagnostics.

use std::{path::PathBuf, sync::Arc, time::Duration};

use arc_trait::arc_trait;
use color_eyre::{Result, eyre::eyre, owo_colors::OwoColorize};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use relative_path::{RelativePath, RelativePathBuf};
use rpkg_rs::resource::{pdefs::PartitionId, resource_partition::PatchId};
use simple_mod_framework_types::{Manifest, ModID, PapayaMap};
use tryvial::try_fn;

use crate::{
	diagnostics::{Diagnostic, DiagnosticTarget},
	game::NominalPartition
};

#[arc_trait]
pub trait Mods {
	fn get_all_mods(&self) -> Result<Vec<ModID>>;
	fn get_mod_root(&self, id: &ModID) -> Option<PathBuf>;
	fn get_mod_manifest(&self, id: &ModID) -> Result<Arc<Manifest>>;
	fn read_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<Vec<u8>>;
	fn read_mod_content_folder(
		&self,
		id: &ModID,
		content_folder: &RelativePath
	) -> Result<Vec<(NominalPartition, Vec<RelativePathBuf>)>>;
	fn read_mod_blob_folder(&self, id: &ModID, blob_folder: &RelativePath) -> Result<Vec<(String, RelativePathBuf)>>;
}

#[arc_trait]
pub trait ModsWritable {
	fn write_mod_manifest(&self, id: &ModID, manifest: Manifest) -> Result<()>;
	fn write_mod_file(&self, id: &ModID, path: &RelativePath, contents: &[u8]) -> Result<()>;
	fn remove_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<()>;
}

#[arc_trait]
pub trait Output {
	fn emit_sdk_mod(&self, name: String, path: PathBuf) -> Result<()>;
	fn emit_package_definition(&self, package_definition: String) -> Result<()>;
	fn emit_thumbs(&self, thumbs: String) -> Result<()>;
	fn emit_rpkg(&self, package: (PartitionId, PatchId), contents: &[u8]) -> Result<()>;
}

#[arc_trait]
pub trait Diagnostics {
	fn emit_diagnostic(&self, diagnostic: Diagnostic) -> Result<()>;
}

#[arc_trait]
pub trait Progress {
	fn start_progress(&self, name: &str, max: u64) -> Result<u64>;

	fn start_spinner(&self, prefix: &str, target: Option<&str>, name: &str) -> Result<u64>;

	fn advance_progress(&self, id: u64, amount: u64) -> Result<()>;

	fn finish_progress(&self, id: u64) -> Result<()>;
}

pub trait World: Diagnostics + Send + Sync + 'static {}

impl<T: Diagnostics + Send + Sync + 'static> World for T {}

#[derive(Debug, Clone)]
pub struct LogDiagnostics;

impl Diagnostics for LogDiagnostics {
	#[try_fn]
	fn emit_diagnostic(&self, diagnostic: Diagnostic) -> Result<()> {
		let target = match diagnostic.target {
			DiagnosticTarget::None => "".to_string(),
			DiagnosticTarget::Mod { mod_id } => mod_id.to_string(),
			DiagnosticTarget::Operation {
				mod_id,
				source,
				script_identifier
			} => {
				format!(
					"{} - {}{}",
					mod_id,
					source,
					if let Some(script_identifier) = script_identifier {
						format!(" ({})", script_identifier)
					} else {
						"".into()
					}
				)
			}
		};

		log::warn!(target: &target, "{}", diagnostic.kind);
	}
}

#[derive(Debug, Clone)]
pub struct CliProgress {
	indicatif: MultiProgress,
	bars: PapayaMap<u64, ProgressBar>
}

impl CliProgress {
	pub fn new(progress: MultiProgress) -> Self {
		Self {
			indicatif: progress,
			bars: Default::default()
		}
	}
}

impl Progress for CliProgress {
	#[try_fn]
	fn start_progress(&self, name: &str, max: u64) -> Result<u64> {
		let progress = self.indicatif.add(
			ProgressBar::new(max)
				.with_message(name.to_owned())
				.with_prefix(">")
				.with_style(
					ProgressStyle::default_bar()
						.template("{prefix:.green} {msg:.blue} {wide_bar} {human_pos}/{human_len}")?
				)
		);

		progress.tick();

		let id = rand::random();
		self.bars.pin().insert(id, progress);
		id
	}

	#[try_fn]
	fn start_spinner(&self, prefix: &str, target: Option<&str>, name: &str) -> Result<u64> {
		let progress = self.indicatif.add(
			ProgressBar::new_spinner()
				.with_style(
					ProgressStyle::default_spinner()
						.template("{spinner:.green} {prefix:.blue} {msg}")?
						.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
				)
				.with_prefix(prefix.to_owned())
				.with_message(if let Some(target) = target {
					format!("{} {}", target.magenta(), name)
				} else {
					name.into()
				})
		);

		progress.enable_steady_tick(Duration::from_millis(100));

		let id = rand::random();
		self.bars.pin().insert(id, progress);
		id
	}

	#[try_fn]
	fn advance_progress(&self, id: u64, amount: u64) -> Result<()> {
		self.bars
			.pin()
			.get(&id)
			.ok_or_else(|| eyre!("No progress with ID {}", id))?
			.inc(amount);
	}

	#[try_fn]
	fn finish_progress(&self, id: u64) -> Result<()> {
		self.indicatif.remove(
			self.bars
				.pin()
				.remove(&id)
				.ok_or_else(|| eyre!("No progress with ID {}", id))?
		);
	}
}

#[derive(Debug, Clone)]
pub struct LogProgress;

impl Progress for LogProgress {
	#[try_fn]
	fn start_progress(&self, name: &str, max: u64) -> Result<u64> {
		log::debug!("{name} (0/{max})");
		rand::random()
	}

	#[try_fn]
	fn start_spinner(&self, prefix: &str, target: Option<&str>, name: &str) -> Result<u64> {
		if let Some(target) = target {
			log::debug!(target: target, "{prefix} {name}");
		} else {
			log::debug!("{prefix} {name}");
		}

		rand::random()
	}

	#[try_fn]
	fn advance_progress(&self, _: u64, _: u64) -> Result<()> {}

	#[try_fn]
	fn finish_progress(&self, _: u64) -> Result<()> {}
}
