use std::{path::PathBuf, sync::Arc};

use color_eyre::eyre::Result;
use ecow::EcoString;
use glacier_commons::game::GlacierGame;
use relative_path::{RelativePath, RelativePathBuf};
use simple_mod_framework::{
	diagnostics::Diagnostic,
	graph::Attribution,
	scripts::{fs_module, install_glacier_bin1_modules, io_module, qn_additions_module, script_context},
	world::{Diagnostics, Mods}
};
use simple_mod_framework_core::game::NominalPartition;
use simple_mod_framework_types::{Manifest, ModID};

const VERSION: &str = "0.14.0";

fn main() {
	rune::cli::Entry::new()
		.about(format_args!("The Rune Language Interpreter {VERSION}"))
		.context(&mut |_| {
			let attribution = Attribution {
				mod_id: EcoString::from("Misc.Mod").try_into().unwrap(),
				source: "main.rn".try_into().unwrap(),
				script_identifier: None
			};

			struct World;

			impl Diagnostics for World {
				fn emit_diagnostic(&self, _: Diagnostic) -> Result<()> {
					unimplemented!()
				}
			}

			impl Mods for World {
				fn get_all_mods(&self) -> Result<Vec<ModID>> {
					unimplemented!()
				}

				fn get_mod_root(&self, _: &ModID) -> Option<PathBuf> {
					unimplemented!()
				}

				fn get_mod_manifest(&self, _: &ModID) -> Result<Arc<Manifest>> {
					unimplemented!()
				}

				fn read_mod_file(&self, _: &ModID, _: &RelativePath) -> Result<Vec<u8>> {
					unimplemented!()
				}

				fn read_mod_content_folder(
					&self,
					_: &ModID,
					_: &RelativePath
				) -> Result<Vec<(NominalPartition, Vec<RelativePathBuf>)>> {
					unimplemented!()
				}

				fn read_mod_blob_folder(&self, _: &ModID, _: &RelativePath) -> Result<Vec<(String, RelativePathBuf)>> {
					unimplemented!()
				}
			}

			let world = Arc::new(World);

			let mut context = script_context().unwrap();
			context
				.install(io_module(world.clone(), attribution.to_owned()).unwrap())
				.unwrap();
			context
				.install(qn_additions_module(world.clone(), attribution.to_owned()).unwrap())
				.unwrap();
			context
				.install(fs_module(world.clone(), attribution.mod_id).unwrap())
				.unwrap();
			install_glacier_bin1_modules(&mut context, GlacierGame::H3).unwrap();
			Ok(context)
		})
		.run();
}
