use std::{fs, panic::catch_unwind, path::Path, process::Command, sync::atomic::Ordering};

use color_eyre::eyre::{OptionExt, Result, WrapErr, bail, eyre};
use fn_wrap_err::wrap_err;
use glacier_bin1::game::h3::{STemplateEntityBlueprint, STemplateEntityFactory};
use glacier_commons::{
	game::GlacierGame,
	hash_list::HASH_LIST,
	metadata::{ResourceMetadata, RuntimeID},
	rpkg_tool::RpkgResourceMeta
};
use quickentity_rs::{
	entity::{CommentEntity, Entity},
	generate_patch
};
use rpkg_rs::resource::partition_manager::PartitionManager;
use serde_json::Value;
use tempfile::TempDir;
use tryvial::try_fn;
use walkdir::WalkDir;

pub trait RunCommandExt {
	/// Run the command, returning its stdout. If the command fails (status code non-zero), an error is returned with the stderr output.
	fn run(&mut self) -> Result<Vec<u8>>;
}

impl RunCommandExt for Command {
	#[try_fn]
	fn run(&mut self) -> Result<Vec<u8>> {
		let output = self.output().wrap_err("Couldn't execute process")?;

		if output.status.success() {
			output.stdout
		} else {
			return Err(eyre!(
				"Command failed with status code {:?}\nStdout:\n{}\n\nStderr:\n{}",
				output.status.code(),
				String::from_utf8_lossy(&output.stdout).trim(),
				String::from_utf8_lossy(&output.stderr).trim()
			));
		}
	}
}

#[wrap_err("Couldn't get partition of resource")]
pub fn get_resource_partition(game_files: &PartitionManager, id: RuntimeID) -> Result<Option<&String>> {
	for partition in &game_files.partitions {
		if partition.get_resource_info(&id.as_u64().into()).is_ok() {
			return Ok(partition.partition_info().name.as_ref());
		}
	}

	Ok(None)
}

fn no_window(command: &mut Command) -> &mut Command {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
	}
	#[cfg(not(windows))]
	{
		command
	}
}

fn catch_panics<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> Result<T> {
	catch_unwind(f).map_err(|e| {
		e.downcast::<String>()
			.map(|s| eyre!(s))
			.or_else(|e| e.downcast::<&str>().map(|s| eyre!(s)))
			.unwrap_or_else(|_| eyre!("No information available"))
	})
}

#[try_fn]
#[wrap_err("Couldn't upgrade entity")]
pub fn upgrade_entity(entity_path: &Path) -> Result<()> {
	if HASH_LIST.version.load(Ordering::SeqCst) == 0 {
		HASH_LIST.load_cached().wrap_err("Couldn't load hash list")?;
	}

	let orig_file: Value = serde_json::from_slice(&fs::read(entity_path).wrap_err("Couldn't read the file")?)
		.wrap_err("Couldn't parse the file as JSON")?;

	let qn_version = orig_file
		.get("quickEntityVersion")
		.ok_or_eyre("Couldn't get quickEntityVersion")?
		.as_f64()
		.ok_or_eyre("quickEntityVersion was not f64")?;

	if qn_version == 3.0 || qn_version == 3.1 {
		let comments = serde_json::from_value::<Vec<CommentEntity>>(
			orig_file
				.get("comments")
				.ok_or_eyre("Couldn't get comments")?
				.to_owned()
		)?;

		let (temp, temp_meta, tblu, tblu_meta) = if qn_version == 3.0 {
			let entity = serde_json::from_value(orig_file).wrap_err("Entity was invalid")?;

			let (temp, temp_meta, tblu, tblu_meta) = catch_panics(|| quickentity_3::convert_to_rt(&entity))
				.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

			(
				serde_json::to_value(&temp)?,
				serde_json::to_value(&temp_meta)?,
				serde_json::to_value(&tblu)?,
				serde_json::to_value(&tblu_meta)?
			)
		} else {
			let entity = serde_json::from_value(orig_file).wrap_err("Entity was invalid")?;

			let (temp, temp_meta, tblu, tblu_meta) =
				quickentity_31::convert_to_rt(&entity).map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

			(
				serde_json::to_value(&temp)?,
				serde_json::to_value(&temp_meta)?,
				serde_json::to_value(&tblu)?,
				serde_json::to_value(&tblu_meta)?
			)
		};

		let mut entity: Entity = serde_json::from_slice(&serde_json::to_vec(
			&Entity::from_game(
				&serde_json::from_value::<STemplateEntityFactory>(temp)
					.wrap_err("Couldn't parse generated TEMP JSON")?,
				&serde_json::from_value::<RpkgResourceMeta>(temp_meta)
					.wrap_err("Couldn't parse generated TEMP meta JSON")?
					.try_into()?,
				&serde_json::from_value(tblu).wrap_err("Couldn't parse generated TBLU JSON")?,
				&serde_json::from_value::<RpkgResourceMeta>(tblu_meta)
					.wrap_err("Couldn't parse generated TBLU meta JSON")?
					.try_into()?,
				false
			)
			.map_err(|x| eyre!("QuickEntity error: {x:?}"))
			.wrap_err("Couldn't convert entity to modern QN")?
		)?)?;

		entity.comments = comments;

		fs::write(entity_path, to_vec_float_format(&entity)?).wrap_err("Couldn't write output")?;
	} else if qn_version < 3.0 {
		let working_dir = TempDir::new()?;
		let working_dir = working_dir.path();

		no_window(&mut Command::new(if cfg!(windows) { "qnjs.exe" } else { "./qnjs" }))
			.args([
				"generate",
				&entity_path.to_string_lossy(),
				&working_dir
					.join(format!(
						"{}.TEMP.json",
						orig_file
							.get("tempHash")
							.and_then(|x| x.as_str())
							.ok_or_eyre("tempHash is not string")?
					))
					.to_string_lossy(),
				&working_dir
					.join(format!(
						"{}.TEMP.meta.JSON",
						orig_file
							.get("tempHash")
							.and_then(|x| x.as_str())
							.ok_or_eyre("tempHash is not string")?
					))
					.to_string_lossy(),
				&working_dir
					.join(format!(
						"{}.TBLU.json",
						orig_file
							.get("tbluHash")
							.and_then(|x| x.as_str())
							.ok_or_eyre("tbluHash is not string")?
					))
					.to_string_lossy(),
				&working_dir
					.join(format!(
						"{}.TBLU.meta.JSON",
						orig_file
							.get("tbluHash")
							.and_then(|x| x.as_str())
							.ok_or_eyre("tbluHash is not string")?
					))
					.to_string_lossy()
			])
			.run()
			.wrap_err("qnjs failed to run")?;

		let comments = orig_file
			.get("entities")
			.ok_or_eyre("Couldn't get entities")?
			.as_object()
			.ok_or_eyre("entities was not an object")?
			.values()
			.filter(|x| {
				x.get("type")
					.is_some_and(|x| x.as_str().is_some_and(|x| x == "comment"))
			})
			.map(|x| {
				Ok(CommentEntity {
					parent: Some(
						x.get("parent")
							.ok_or_eyre("Couldn't get comment parent")?
							.as_str()
							.ok_or_eyre("Comment parent was not a string")?
							.to_owned()
							.parse()?
					),
					name: x
						.get("name")
						.ok_or_eyre("Couldn't get comment name")?
						.as_str()
						.ok_or_eyre("Comment name was not a string")?
						.into(),
					text: x
						.get("text")
						.ok_or_eyre("Couldn't get comment text")?
						.as_str()
						.ok_or_eyre("Comment text was not a string")?
						.into()
				})
			})
			.collect::<Result<Vec<_>>>()?;

		let mut entity: Entity = serde_json::from_slice(&serde_json::to_vec(
			&Entity::from_game(
				&serde_json::from_slice::<STemplateEntityFactory>(
					&fs::read(working_dir.join(format!(
                        "{}.TEMP.json",
                        orig_file
                            .get("tempHash")
                            .and_then(|x| x.as_str())
                            .ok_or_eyre("tempHash is not string")?
                    )))
					.wrap_err("Couldn't read ResourceTool TEMP JSON")?
				)
				.wrap_err("Couldn't parse ResourceTool TEMP JSON")?,
				&serde_json::from_slice::<RpkgResourceMeta>(
					&fs::read(working_dir.join(format!(
                        "{}.TEMP.meta.JSON",
                        orig_file
                            .get("tempHash")
                            .and_then(|x| x.as_str())
                            .ok_or_eyre("tempHash is not string")?
                    )))
					.wrap_err("Couldn't read TEMP meta JSON")?
				)
				.wrap_err("Couldn't parse TEMP meta JSON")?
				.try_into()?,
				&serde_json::from_slice(
					&fs::read(working_dir.join(format!(
                        "{}.TBLU.json",
                        orig_file
                            .get("tbluHash")
                            .and_then(|x| x.as_str())
                            .ok_or_eyre("tbluHash is not string")?
                    )))
					.wrap_err("Couldn't read ResourceTool TBLU JSON")?
				)
				.wrap_err("Couldn't parse ResourceTool TBLU JSON")?,
				&serde_json::from_slice::<RpkgResourceMeta>(
					&fs::read(working_dir.join(format!(
                        "{}.TBLU.meta.JSON",
                        orig_file
                            .get("tbluHash")
                            .and_then(|x| x.as_str())
                            .ok_or_eyre("tbluHash is not string")?
                    )))
					.wrap_err("Couldn't read TBLU meta JSON")?
				)
				.wrap_err("Couldn't parse TBLU meta JSON")?
				.try_into()?,
				false
			)
			.map_err(|x| eyre!("QuickEntity error: {x:?}"))
			.wrap_err("Couldn't convert entity to modern QN")?
		)?)?;

		entity.comments = comments;

		fs::write(entity_path, to_vec_float_format(&entity)?).wrap_err("Couldn't write output")?;
	}
}

#[try_fn]
#[wrap_err("Couldn't upgrade patch")]
pub fn upgrade_patch(partition_manager: &PartitionManager, mod_path: &Path, patch_path: &Path) -> Result<()> {
	if HASH_LIST.version.load(Ordering::SeqCst) == 0 {
		HASH_LIST.load_cached().wrap_err("Couldn't load hash list")?;
	}

	let orig_file: Value = serde_json::from_slice(&fs::read(patch_path).wrap_err("Couldn't read the file")?)
		.wrap_err("Couldn't parse the file as JSON")?;

	let qn_version = orig_file
		.get("patchVersion")
		.ok_or_eyre("Couldn't get patchVersion")?
		.as_u64()
		.ok_or_eyre("patchVersion was not u64")?;

	if qn_version >= quickentity_rs::PATCH_VERSION as u64 {
		return Ok(());
	}

	let fac_id = orig_file
		.get("tempHash")
		.ok_or_eyre("tempHash nonexistent")?
		.as_str()
		.ok_or_eyre("tempHash not string")?
		.parse()?;

	let fac_partition = get_resource_partition(partition_manager, fac_id)?;

	let (vanilla_fac_meta, vanilla_fac, vanilla_blu_meta, vanilla_blu) = if let Some(fac_partition) = fac_partition {
		let blu_id = orig_file
			.get("tbluHash")
			.ok_or_eyre("tbluHash nonexistent")?
			.as_str()
			.ok_or_eyre("tbluHash not string")?
			.parse()?;

		let blu_partition = get_resource_partition(partition_manager, blu_id)?.ok_or_eyre("TBLU file not in game")?;

		let vanilla_fac_res = (
			ResourceMetadata::try_from(
				partition_manager
					.partitions
					.iter()
					.find(|x| x.partition_info().name.as_ref() == Some(fac_partition))
					.unwrap()
					.get_resource_info(&fac_id.as_u64().into())?
			)?,
			partition_manager
				.partitions
				.iter()
				.find(|x| x.partition_info().name.as_ref() == Some(fac_partition))
				.unwrap()
				.read_resource(&fac_id.as_u64().into())?
		);

		let vanilla_blu_res = (
			ResourceMetadata::try_from(
				partition_manager
					.partitions
					.iter()
					.find(|x| x.partition_info().name.as_ref() == Some(blu_partition))
					.unwrap()
					.get_resource_info(&blu_id.as_u64().into())?
			)?,
			partition_manager
				.partitions
				.iter()
				.find(|x| x.partition_info().name.as_ref() == Some(blu_partition))
				.unwrap()
				.read_resource(&blu_id.as_u64().into())?
		);

		let (vanilla_fac, vanilla_blu) = rayon::join(
			|| glacier_bin1::deserialize::<STemplateEntityFactory>(&vanilla_fac_res.1),
			|| glacier_bin1::deserialize::<STemplateEntityBlueprint>(&vanilla_blu_res.1)
		);

		let (vanilla_fac, vanilla_blu) = (vanilla_fac?, vanilla_blu?);

		(vanilla_fac_res.0, vanilla_fac, vanilla_blu_res.0, vanilla_blu)
	} else {
		// Attempt to find the entity somewhere else in the mod
		let temp_hash = orig_file
			.get("tempHash")
			.ok_or_eyre("tempHash nonexistent")?
			.as_str()
			.ok_or_eyre("tempHash not string")?
			.to_owned();

		let Some(found) = WalkDir::new(mod_path)
			.sort_by_file_name()
			.into_iter()
			.flatten()
			.try_find(|entry| {
				if entry.file_type().is_file() && entry.path().to_string_lossy().ends_with(".entity.json") {
					let entity =
						serde_json::from_slice::<Value>(&fs::read(entry.path()).wrap_err("Couldn't read entity file")?)
							.wrap_err("Couldn't parse entity file")?;

					if let Some(value) = entity.get("tempHash") {
						if value.as_str().ok_or_eyre("tempHash was not string")? == temp_hash {
							// Found matching but not upgraded entity
							upgrade_entity(entry.path())?;
							return Ok(true);
						}
					} else if serde_json::from_value::<RuntimeID>(
						entity
							.get("factory")
							.ok_or_eyre("No factory or tempHash field on entity.json")?
							.to_owned()
					)? == RuntimeID::from_hash(&temp_hash)?
					{
						// Found matching and already upgraded entity
						return Ok(true);
					}
				}

				color_eyre::eyre::Ok(false)
			})?
		else {
			bail!("Couldn't find entity {} in game or mod files", temp_hash);
		};

		let entity =
			serde_json::from_slice::<Entity>(&fs::read(found.path()).wrap_err("Couldn't read found entity file")?)
				.wrap_err("Couldn't parse found entity file")?;

		let (fac, fac_meta, blu, blu_meta) = entity
			.to_game()
			.map_err(|x| eyre!("QuickEntity error: {:?}", x))
			.wrap_err("Couldn't convert entity.json!")?;

		(fac_meta, fac, blu_meta, blu)
	};

	if qn_version == 6 {
		let old_original = quickentity_31::convert_to_qn(
			&serde_json::from_value(serde_json::to_value(&vanilla_fac)?)?,
			&serde_json::from_value(serde_json::to_value(RpkgResourceMeta::from_resource_metadata(
				vanilla_fac_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
				false
			))?)?,
			&serde_json::from_value(serde_json::to_value(&vanilla_blu)?)?,
			&serde_json::from_value(serde_json::to_value(RpkgResourceMeta::from_resource_metadata(
				vanilla_blu_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
				false
			))?)?,
			true
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let mut old_patched = old_original.to_owned();

		quickentity_31::apply_patch(
			&mut old_patched,
			serde_json::from_value(orig_file).wrap_err("Patch was invalid")?,
			true
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let comments = serde_json::from_value::<Vec<CommentEntity>>(serde_json::to_value(&old_patched.comments)?)?;

		let (orig_temp, orig_temp_meta, orig_tblu, orig_tblu_meta) =
			quickentity_31::convert_to_rt(&old_original).map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let modern_original = Entity::from_game(
			&serde_json::from_value::<STemplateEntityFactory>(serde_json::to_value(&orig_temp)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&orig_temp_meta)?)
				.wrap_err("Couldn't parse TEMP meta JSON!")?
				.try_into()?,
			&serde_json::from_value(serde_json::to_value(&orig_tblu)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&orig_tblu_meta)?)
				.wrap_err("Couldn't parse TBLU meta JSON!")?
				.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert original entity to modern QN!")?;

		let (patched_temp, patched_temp_meta, patched_tblu, patched_tblu_meta) =
			quickentity_31::convert_to_rt(&old_patched).map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let mut modern_patched = Entity::from_game(
			&serde_json::from_value::<STemplateEntityFactory>(serde_json::to_value(&patched_temp)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&patched_temp_meta)?)
				.wrap_err("Couldn't parse TEMP meta JSON!")?
				.try_into()?,
			&serde_json::from_value(serde_json::to_value(&patched_tblu)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&patched_tblu_meta)?)
				.wrap_err("Couldn't parse TBLU meta JSON!")?
				.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert patched entity to modern QN!")?;

		modern_patched.comments = comments;

		let patch = generate_patch(&modern_original, &modern_patched)
			.map_err(|x| eyre!("QuickEntity error: {:?}", x))
			.wrap_err("Couldn't generate new patch!")?;

		fs::write(patch_path, to_vec_float_format(&patch)?).wrap_err("Couldn't write output")?;
	} else if qn_version == 5 {
		let fac = serde_json::from_value(serde_json::to_value(&vanilla_fac)?)?;
		let blu = serde_json::from_value(serde_json::to_value(&vanilla_blu)?)?;

		let old_original = catch_panics(|| {
			color_eyre::eyre::Ok(quickentity_3::convert_to_qn(
				&fac,
				&serde_json::from_value(serde_json::to_value(RpkgResourceMeta::from_resource_metadata(
					vanilla_fac_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
					false
				))?)?,
				&blu,
				&serde_json::from_value(serde_json::to_value(RpkgResourceMeta::from_resource_metadata(
					vanilla_blu_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
					false
				))?)?,
				true
			))
		})
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))??;

		let patch = serde_json::from_value(orig_file).wrap_err("Patch was invalid")?;

		let old_patched = catch_panics(|| {
			let mut old_patched = serde_json::to_value(&old_original)?;
			quickentity_3::apply_patch(&mut old_patched, &patch);
			color_eyre::eyre::Ok(old_patched)
		})
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))??;

		let comments = serde_json::from_value::<Vec<CommentEntity>>(
			old_patched
				.get("comments")
				.ok_or_eyre("Couldn't get comments")?
				.to_owned()
		)?;

		let (orig_temp, orig_temp_meta, orig_tblu, orig_tblu_meta) =
			catch_panics(|| quickentity_3::convert_to_rt(&old_original))
				.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let modern_original = Entity::from_game(
			&serde_json::from_value::<STemplateEntityFactory>(serde_json::to_value(&orig_temp)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&orig_temp_meta)?)
				.wrap_err("Couldn't parse TEMP meta JSON!")?
				.try_into()?,
			&serde_json::from_value(serde_json::to_value(&orig_tblu)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&orig_tblu_meta)?)
				.wrap_err("Couldn't parse TBLU meta JSON!")?
				.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert original entity to modern QN!")?;

		let old_patched = serde_json::from_value(old_patched).wrap_err("Couldn't parse patched entity")?;

		let (patched_temp, patched_temp_meta, patched_tblu, patched_tblu_meta) =
			catch_panics(|| quickentity_3::convert_to_rt(&old_patched))
				.map_err(|x| eyre!("QuickEntity error: {:?}", x))?;

		let mut modern_patched = Entity::from_game(
			&serde_json::from_value::<STemplateEntityFactory>(serde_json::to_value(&patched_temp)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&patched_temp_meta)?)
				.wrap_err("Couldn't parse TEMP meta JSON!")?
				.try_into()?,
			&serde_json::from_value(serde_json::to_value(&patched_tblu)?)?,
			&serde_json::from_value::<RpkgResourceMeta>(serde_json::to_value(&patched_tblu_meta)?)
				.wrap_err("Couldn't parse TBLU meta JSON!")?
				.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert patched entity to modern QN!")?;

		modern_patched.comments = comments;

		let patch = generate_patch(&modern_original, &modern_patched)
			.map_err(|x| eyre!("QuickEntity error: {:?}", x))
			.wrap_err("Couldn't generate new patch!")?;

		fs::write(patch_path, to_vec_float_format(&patch)?).wrap_err("Couldn't write output")?;
	} else if qn_version < 5 {
		let working_dir = TempDir::new()?;
		let working_dir = working_dir.path();

		fs::write(
			working_dir.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash())),
			serde_json::to_vec(&vanilla_fac)?
		)
		.wrap_err("Couldn't write TEMP JSON")?;

		fs::write(
			working_dir.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash())),
			serde_json::to_vec(&RpkgResourceMeta::from_resource_metadata(
				vanilla_fac_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
				false
			))?
		)
		.wrap_err("Couldn't write TEMP.meta JSON")?;

		fs::write(
			working_dir.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash())),
			serde_json::to_vec(&vanilla_blu)?
		)
		.wrap_err("Couldn't write TBLU JSON")?;

		fs::write(
			working_dir.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash())),
			serde_json::to_vec(&RpkgResourceMeta::from_resource_metadata(
				vanilla_blu_meta.to_owned().to_extended(&[0; 12], GlacierGame::H3)?,
				false
			))?
		)
		.wrap_err("Couldn't write TBLU.meta JSON")?;

		no_window(&mut Command::new(if cfg!(windows) { "qnjs.exe" } else { "./qnjs" }))
			.args([
				"convert",
				&working_dir.join("old_original.json").to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy(),
				match qn_version as u32 {
					4 => "2.1",
					3 => "2",
					_ => "1.136"
				}
			])
			.run()
			.wrap_err("qnjs failed to run!")?;

		no_window(&mut Command::new(if cfg!(windows) { "qnjs.exe" } else { "./qnjs" }))
			.args([
				"applyPatch",
				&working_dir.join("old_patched.json").to_string_lossy(),
				&working_dir.join("old_original.json").to_string_lossy(),
				&patch_path.to_string_lossy()
			])
			.run()
			.wrap_err("qnjs failed to run!")?;

		let comments = serde_json::from_slice::<Value>(
			&fs::read(working_dir.join("old_patched.json")).wrap_err("Couldn't read old_patched.json")?
		)
		.wrap_err("Couldn't parse old_patched.json")?
		.get("entities")
		.ok_or_eyre("Couldn't get entities")?
		.as_object()
		.ok_or_eyre("entities was not an object")?
		.values()
		.filter(|x| {
			x.get("type")
				.is_some_and(|x| x.as_str().is_some_and(|x| x == "comment"))
		})
		.map(|x| {
			Ok(CommentEntity {
				parent: Some(
					x.get("parent")
						.ok_or_eyre("Couldn't get comment parent")?
						.as_str()
						.ok_or_eyre("Comment parent was not a string")?
						.to_owned()
						.parse()?
				),
				name: x
					.get("name")
					.ok_or_eyre("Couldn't get comment name")?
					.as_str()
					.ok_or_eyre("Comment name was not a string")?
					.into(),
				text: x
					.get("text")
					.ok_or_eyre("Couldn't get comment text")?
					.as_str()
					.ok_or_eyre("Comment text was not a string")?
					.into()
			})
		})
		.collect::<Result<Vec<_>>>()?;

		no_window(&mut Command::new(if cfg!(windows) { "qnjs.exe" } else { "./qnjs" }))
			.args([
				"generate",
				&working_dir.join("old_original.json").to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy()
			])
			.run()
			.wrap_err("qnjs failed to run!")?;

		let modern_original = Entity::from_game(
			&serde_json::from_slice::<STemplateEntityFactory>(
				&fs::read(working_dir.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash())))
					.wrap_err("Couldn't read ResourceTool TEMP JSON!")?
			)
			.wrap_err("Couldn't parse ResourceTool TEMP JSON!")?,
			&serde_json::from_slice::<RpkgResourceMeta>(
				&fs::read(working_dir.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash())))
					.wrap_err("Couldn't read TEMP meta JSON!")?
			)
			.wrap_err("Couldn't parse TEMP meta JSON!")?
			.try_into()?,
			&serde_json::from_slice(
				&fs::read(working_dir.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash())))
					.wrap_err("Couldn't read ResourceTool TBLU JSON!")?
			)
			.wrap_err("Couldn't parse ResourceTool TBLU JSON!")?,
			&serde_json::from_slice::<RpkgResourceMeta>(
				&fs::read(working_dir.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash())))
					.wrap_err("Couldn't read TBLU meta JSON!")?
			)
			.wrap_err("Couldn't parse TBLU meta JSON!")?
			.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert original entity to modern QN!")?;

		no_window(&mut Command::new(if cfg!(windows) { "qnjs.exe" } else { "./qnjs" }))
			.args([
				"generate",
				&working_dir.join("old_patched.json").to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy(),
				&working_dir
					.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash()))
					.to_string_lossy()
			])
			.run()
			.wrap_err("qnjs failed to run!")?;

		let mut modern_patched = Entity::from_game(
			&serde_json::from_slice::<STemplateEntityFactory>(
				&fs::read(working_dir.join(format!("{}.TEMP.json", vanilla_fac_meta.id.to_hash())))
					.wrap_err("Couldn't read ResourceTool TEMP JSON!")?
			)
			.wrap_err("Couldn't parse ResourceTool TEMP JSON!")?,
			&serde_json::from_slice::<RpkgResourceMeta>(
				&fs::read(working_dir.join(format!("{}.TEMP.meta.JSON", vanilla_fac_meta.id.to_hash())))
					.wrap_err("Couldn't read TEMP meta JSON!")?
			)
			.wrap_err("Couldn't parse TEMP meta JSON!")?
			.try_into()?,
			&serde_json::from_slice(
				&fs::read(working_dir.join(format!("{}.TBLU.json", vanilla_blu_meta.id.to_hash())))
					.wrap_err("Couldn't read ResourceTool TBLU JSON!")?
			)
			.wrap_err("Couldn't parse ResourceTool TBLU JSON!")?,
			&serde_json::from_slice::<RpkgResourceMeta>(
				&fs::read(working_dir.join(format!("{}.TBLU.meta.JSON", vanilla_blu_meta.id.to_hash())))
					.wrap_err("Couldn't read TBLU meta JSON!")?
			)
			.wrap_err("Couldn't parse TBLU meta JSON!")?
			.try_into()?,
			false
		)
		.map_err(|x| eyre!("QuickEntity error: {:?}", x))
		.wrap_err("Couldn't convert patched entity to modern QN!")?;

		modern_patched.comments = comments;

		let patch = generate_patch(&modern_original, &modern_patched)
			.map_err(|x| eyre!("QuickEntity error: {:?}", x))
			.wrap_err("Couldn't generate new patch!")?;

		fs::write(patch_path, to_vec_float_format(&patch)?).wrap_err("Couldn't write output")?;
	}
}

// Below is from QuickEntity source code
pub fn to_vec_float_format<W>(contents: &W) -> Result<Vec<u8>>
where
	W: ?Sized + serde::Serialize
{
	let mut writer = Vec::with_capacity(128);

	let mut ser = serde_json::Serializer::with_formatter(&mut writer, FloatFormatter);
	contents.serialize(&mut ser)?;

	Ok(writer)
}

#[derive(Clone, Debug)]
struct FloatFormatter;

impl serde_json::ser::Formatter for FloatFormatter {
	#[inline]
	fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> std::io::Result<()>
	where
		W: ?Sized + std::io::Write
	{
		writer.write_all(value.to_string().as_bytes())
	}

	#[inline]
	fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()>
	where
		W: ?Sized + std::io::Write
	{
		writer.write_all(value.to_string().as_bytes())
	}

	/// Writes a number that has already been rendered to a string.
	#[inline]
	fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> std::io::Result<()>
	where
		W: ?Sized + std::io::Write
	{
		let x = value.parse::<f64>();
		if let Ok(y) = x {
			if value.parse::<u64>().is_err() || y.to_string() == value.parse::<u64>().unwrap().to_string() {
				writer
					.write_all(
						if y.to_string() == "-0" {
							"0".to_string()
						} else {
							y.to_string()
						}
						.as_bytes()
					)
					.unwrap();
			} else {
				writer.write_all(value.as_bytes()).unwrap();
			}
		} else {
			writer.write_all(value.as_bytes()).unwrap();
		}

		Ok(())
	}
}
