#![feature(try_blocks)]

use std::{fs, path::Path, sync::Arc};

use color_eyre::{
	Result,
	eyre::{OptionExt, bail, eyre}
};
use divan::{AllocProfiler, Bencher, black_box};
use glacier_commons::{
	game::GlacierGame,
	hash_list::HASH_LIST,
	metadata::{ResourceMetadata, RuntimeID}
};
use itertools::Itertools;
use mimalloc::MiMalloc;
use relative_path::PathExt;
use rpkg_diff::ResourceDiff;
use rpkg_rs::resource::{pdefs::PackageDefinitionSource, resource_package::ResourcePackage};
use serde_json::from_slice;
use simple_mod_framework::{
	state::RealPartition,
	world::{LogDiagnostics, LogProgress}
};
use simple_mod_framework_core::game::{GameContext, detect_game};
use simple_mod_framework_types::{Config, HashMap, HashSet};
use tryvial::try_fn;

#[global_allocator]
static ALLOC: AllocProfiler<MiMalloc> = AllocProfiler::new(MiMalloc);

fn main() {
	std::env::set_current_dir({
		let mut x = std::env::current_exe().unwrap();
		x.pop();
		x.join("../../../simple-mod-framework/benches/corpus")
	})
	.unwrap();

	divan::main();
}

#[try_fn]
fn get_game() -> Result<GameContext> {
	let config: Config = from_slice(&fs::read("config.json")?)?;

	let game_path = config.game_path.ok_or_eyre("No game path set")?;

	HASH_LIST.load_cached()?;

	GameContext::from_game(
		detect_game(game_path)?.ok_or_eyre("No game detected")?,
		&LogDiagnostics,
		false
	)?
}

#[try_fn]
fn diff_dirs(first: &Path, second: &Path) -> Result<()> {
	let first_contents = fs::read_dir(first)?
		.map(|x| x.map(|x| x.path()))
		.collect::<Result<Vec<_>, _>>()?;

	let second_contents = fs::read_dir(second)?
		.map(|x| x.map(|x| x.path()))
		.collect::<Result<Vec<_>, _>>()?;

	if first_contents.len() != second_contents.len() {
		if let Some(first_only) = first_contents.iter().find(|x| !second_contents.contains(x)) {
			bail!("File {} only in first dir", first_only.to_string_lossy());
		} else if let Some(second_only) = second_contents.iter().find(|x| !first_contents.contains(x)) {
			bail!("File {} only in second dir", second_only.to_string_lossy());
		}

		unreachable!();
	}

	for (path1, path2) in first_contents.iter().zip(second_contents.iter()) {
		if path1.relative_to(first)? != path2.relative_to(second)? {
			bail!(
				"File path mismatch between {} and {}",
				path1.to_string_lossy(),
				path2.to_string_lossy()
			);
		}

		if path1.is_dir() && path2.is_dir() {
			diff_dirs(path1, path2)?;
		} else if path1.is_file() && path2.is_file() {
			if path1.extension().ok_or_eyre("No file extension")?
				!= path2.extension().ok_or_eyre("No file extension")?
			{
				bail!(
					"File extension mismatch between {} and {}",
					path1.to_string_lossy(),
					path2.to_string_lossy()
				);
			}

			let rel_path = path1.relative_to(first)?;

			let extension = path1.extension().ok_or_eyre("No file extension")?.to_string_lossy();
			if extension == "rpkg" {
				let rpkg1 = ResourcePackage::from_file(path1, GlacierGame::H3.into())?;
				let rpkg2 = ResourcePackage::from_file(path2, GlacierGame::H3.into())?;

				for id in rpkg1.resources().keys().chain(rpkg2.resources().keys()).unique() {
					let first = rpkg1.resources().get(id);
					let second = rpkg2.resources().get(id);
					if first.is_some() && second.is_none() {
						bail!(
							"Resource only in first RPKG for {rel_path}: {}",
							RuntimeID::try_from(id)?
						);
					} else if first.is_none() && second.is_some() {
						bail!(
							"Resource only in second RPKG for {rel_path}: {}",
							RuntimeID::try_from(id)?
						);
					} else {
						let (first, second) = (first.unwrap(), second.unwrap());
						let (first, second) = (ResourceMetadata::try_from(first)?, ResourceMetadata::try_from(second)?);

						let first_data = rpkg1.read_resource(id)?;
						let second_data = rpkg2.read_resource(id)?;

						if let ResourceDiff::Different { description } =
							rpkg_diff::diff(&first, &first_data, &second, &second_data)
								.map_err(|e| eyre!("Error computing diff for {id}: {e:?}"))?
						{
							bail!(
								"Resource {} in {rel_path} differs: {}",
								RuntimeID::try_from(id)?,
								description.as_deref().unwrap_or("binary files differ")
							);
						}
					}
				}
			} else if extension == "txt" {
				let contents1 = fs::read(path1)?;
				let contents2 = fs::read(path2)?;

				let pdef1 = PackageDefinitionSource::HM3(contents1).read()?;
				let pdef2 = PackageDefinitionSource::HM3(contents2).read()?;

				if pdef1.len() != pdef2.len() {
					bail!(
						"Package definition length mismatch between {} and {}",
						path1.to_string_lossy(),
						path2.to_string_lossy()
					);
				}

				for (entry1, entry2) in pdef1.iter().zip(pdef2.iter()) {
					if entry1.id != entry2.id
						|| entry1.name != entry2.name
						|| entry1.parent != entry2.parent
						|| entry1.patch_level != entry2.patch_level
					{
						bail!(
							"Package definition entry mismatch between {} and {}: {entry1:?} vs {entry2:?}",
							path1.to_string_lossy(),
							path2.to_string_lossy()
						);
					}

					if entry1.roots.iter().collect::<HashSet<_>>() != entry2.roots.iter().collect::<HashSet<_>>() {
						bail!(
							"Package definition roots mismatch for partition {} ({:?}): {:?} vs {:?}",
							entry1.id,
							entry1.name,
							entry1.roots,
							entry2.roots
						);
					}
				}
			} else {
				let contents1 = fs::read(path1)?;
				let contents2 = fs::read(path2)?;
				if contents1 != contents2 {
					bail!(
						"File contents mismatch between {} and {}",
						path1.to_string_lossy(),
						path2.to_string_lossy()
					);
				}
			}
		} else {
			bail!(
				"File type mismatch between {} and {}",
				path1.to_string_lossy(),
				path2.to_string_lossy()
			);
		}
	}
}

#[divan::bench(sample_count = 5, sample_size = 1)]
fn deploy(bencher: Bencher) {
	bencher
		.with_inputs(|| {
			let _ = fs::remove_dir_all("cache");
		})
		.bench_values(|_| {
			tokio::runtime::Runtime::new()
				.unwrap()
				.block_on(simple_mod_framework::run::deploy(Arc::new(LogProgress)))
				.unwrap();

			if Path::new("expected").exists() {
				diff_dirs(Path::new("expected"), Path::new("output")).unwrap();
			}
		});
}

#[divan::bench(
	args = [
		(RuntimeID::from_path("[assembly:/_pro/scenes/missions/marrakesh/location.brick].pc_entitytype"), RealPartition("wet".into())),
		(RuntimeID::from_path("[assembly:/_pro/scenes/missions/coastaltown/_scene_octopus.entity].pc_entitytemplate"), RealPartition("super".into())),
		(RuntimeID::from_path("[assembly:/_pro/scenes/missions/wet/scenario_rat.brick].pc_entitytype"), RealPartition("legacy".into())),
		(RuntimeID::from_path("[assembly:/_pro/environment/templates/backdrop/sky/sky_clear.template?/sky_clear_b.entitytemplate].pc_entitytype"), RealPartition("super".into())),
		(RuntimeID::from_path("[assembly:/_pro/scenes/missions/golden/mission_gecko/scenario_gecko.brick].pc_entitytype"), RealPartition("legacy".into())),
		(RuntimeID::from_path("[assembly:/_pro/scenes/missions/wet/scenario_rat.brick].pc_entitytype"), RealPartition("wet".into()))
	],
	sample_count=500
)]
fn get_portable_references(bencher: Bencher, &(resource, ref for_partition): &(RuntimeID, RealPartition)) {
	let game = get_game().unwrap();

	let partition_hierarchies = game
		.game_files
		.partitions
		.iter()
		.map(|x| x.partition_info().name.as_deref().unwrap_or_default())
		.map(|for_partition| {
			Ok((
				RealPartition(for_partition.into()),
				game.get_accessible_partitions(for_partition)?
			))
		})
		.collect::<Result<HashMap<_, _>>>()
		.unwrap();

	bencher.bench(|| {
		let to_port = &papaya::HashSet::default();

		simple_mod_framework::deploy::get_portable_references(
			&game,
			&Default::default(),
			&Default::default(),
			black_box(resource),
			partition_hierarchies.get(for_partition).unwrap(),
			(to_port, &to_port.owned_guard())
		)
		.unwrap();
	});
}
