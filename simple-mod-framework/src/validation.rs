use std::{fs, path::Path, sync::LazyLock};

use color_eyre::eyre::{Result, WrapErr};
use glacier_formats::{
	material::{MaterialEntity, MaterialInstance},
	sdef::SoundDefinitions
};
use jsonschema::Validator;
use quickentity_rs::{entity::Entity, patch::Patch};
use relative_path::{PathExt, RelativePath};
use serde_json::{Deserializer, from_value, json};
use simple_mod_framework_core::H3_VERSION;
use simple_mod_framework_types::{
	HashSet, Manifest, ManifestData, ModOption, ModOptionData, ModOptionID, ModOptionValue, ValidationResult
};
use tracing::instrument;
use tryvial::try_fn;
use walkdir::WalkDir;

use crate::graph::misc::AspectEntity;

macro_rules! fail {
	($($arg:tt)*) => {
		Ok(ValidationResult::Fail(format!($($arg)*)))
	};
}

pub static CONTRACT_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
	Validator::new(&serde_json::from_str(include_str!("../assets/schemas/contract.json")).unwrap()).unwrap()
});

pub static JSON_PATCH_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
	Validator::new(&serde_json::from_str(include_str!("../assets/schemas/json-patch.json")).unwrap()).unwrap()
});

pub static REPOSITORY_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
	Validator::new(&serde_json::from_str(include_str!("../assets/schemas/repository.json")).unwrap()).unwrap()
});

pub static UNLOCKABLES_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
	Validator::new(&serde_json::from_str(include_str!("../assets/schemas/unlockables.json")).unwrap()).unwrap()
});

pub static LOCALISATION_PATCH_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
	Validator::new(&serde_json::from_str(include_str!("../assets/schemas/localisation-patch.json")).unwrap()).unwrap()
});

#[instrument(skip(mod_root), fields(mod_root = mod_root.as_ref().to_string_lossy().to_string()))]
#[try_fn]
pub fn validate_manifest_data(mod_root: impl AsRef<Path>, data: ManifestData) -> Result<ValidationResult> {
	for content_folder_rel in data.content_folders {
		let content_folder = content_folder_rel.to_logical_path(&mod_root);

		match check_path_exists(&mod_root, &*content_folder_rel)?.wrap_fail("Content folder is invalid") {
			ValidationResult::Pass => {}
			failure @ ValidationResult::Fail(_) => return Ok(failure)
		}

		if content_folder
			.read_dir()
			.wrap_err("Couldn't read content folder entries")?
			.next()
			.is_none()
		{
			return fail!("Content folder {} is empty", *content_folder_rel);
		}

		for partition_folder in content_folder
			.read_dir()
			.wrap_err("Couldn't read content folder entries")?
		{
			let partition_folder = partition_folder?;

			if partition_folder.file_type()?.is_dir() {
				let walkdir = WalkDir::new(partition_folder.path())
					.sort_by_file_name()
					.into_iter()
					.collect::<Result<Vec<_>, _>>()?;

				for file in walkdir.into_iter().filter(|x| x.file_type().is_file()) {
					match file.file_name().to_string_lossy().to_string() {
						filename if filename.ends_with(".entity.patch.json") => {
							if let Err(e) = serde_path_to_error::deserialize::<_, Patch>(&mut Deserializer::from_slice(
								&fs::read(file.path())?
							)) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".material.json") => {
							if let Err(e) = serde_path_to_error::deserialize::<_, MaterialInstance>(
								&mut Deserializer::from_slice(&fs::read(file.path())?)
							) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".material.entity.json") => {
							if let Err(e) = serde_path_to_error::deserialize::<_, MaterialEntity>(
								&mut Deserializer::from_slice(&fs::read(file.path())?)
							) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".aspect.entity.json") => {
							if let Err(e) = serde_path_to_error::deserialize::<_, AspectEntity>(
								&mut Deserializer::from_slice(&fs::read(file.path())?)
							) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".entity.json") => {
							if let Err(e) = serde_path_to_error::deserialize::<_, Entity>(
								&mut Deserializer::from_slice(&fs::read(file.path())?)
							) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename
							if filename.ends_with(".sounddefs.json") || filename.ends_with(".sounddefs.patch.json") =>
						{
							if let Err(e) = serde_path_to_error::deserialize::<_, SoundDefinitions>(
								&mut Deserializer::from_slice(&fs::read(file.path())?)
							) {
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".repository.json") => {
							if let Err(e) =
								REPOSITORY_SCHEMA.validate(&serde_json::from_slice(&fs::read(file.path())?)?)
							{
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".unlockables.json") => {
							if let Err(e) =
								UNLOCKABLES_SCHEMA.validate(&serde_json::from_slice(&fs::read(file.path())?)?)
							{
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".JSON.patch.json") => {
							if let Err(e) =
								JSON_PATCH_SCHEMA.validate(&serde_json::from_slice(&fs::read(file.path())?)?)
							{
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".contract.json") => {
							if let Err(e) = CONTRACT_SCHEMA.validate(&serde_json::from_slice(&fs::read(file.path())?)?)
							{
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						filename if filename.ends_with(".localisation.patch.json") => {
							if let Err(e) =
								LOCALISATION_PATCH_SCHEMA.validate(&serde_json::from_slice(&fs::read(file.path())?)?)
							{
								return fail!("Validation error for {}: {}", file.path().relative_to(&mod_root)?, e);
							}
						}

						_ => {}
					}
				}
			} else {
				return fail!(
					"Content folder {} should only contain partition folders",
					*content_folder_rel
				);
			}
		}
	}

	for blob_folder_rel in data.blob_folders {
		let blob_folder = blob_folder_rel.to_logical_path(&mod_root);

		match check_path_exists(&mod_root, &*blob_folder_rel)?.wrap_fail("Blob folder is invalid") {
			ValidationResult::Pass => {}
			failure @ ValidationResult::Fail(_) => return Ok(failure)
		}

		if blob_folder
			.read_dir()
			.wrap_err("Couldn't read blob folder entries")?
			.next()
			.is_none()
		{
			return fail!("Blob folder {} is empty", *blob_folder_rel);
		}
	}

	for (string, localisation) in data.localisation.iter() {
		if localisation.first_specified().is_none() {
			return fail!("Localisation string {string} is not defined for any language");
		}
	}

	for file in data.peacock_plugins {
		match check_path_exists(&mod_root, &*file)?.wrap_fail("Peacock plugin is invalid") {
			ValidationResult::Pass => {}
			failure @ ValidationResult::Fail(_) => return Ok(failure)
		}
	}

	for (_, files) in data.sdk_mods {
		for file in files {
			match check_path_exists(&mod_root, &*file)?.wrap_fail("SDK mod is invalid") {
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}
		}
	}

	for file in data.scripts {
		let file_path = file.to_logical_path(&mod_root);

		match check_path_exists(&mod_root, &*file)?.wrap_fail("Script is invalid") {
			ValidationResult::Pass => {}
			failure @ ValidationResult::Fail(_) => return Ok(failure)
		}

		let contents = fs::read_to_string(file_path)?;
		if !contents.contains("pub fn data(") && !contents.contains("pub fn operations(") {
			return fail!("Script {} has no functionality", *file);
		}
	}

	ValidationResult::Pass
}

#[instrument(skip(mod_root), fields(mod_root = mod_root.as_ref().to_string_lossy().to_string()))]
#[try_fn]
pub fn validate_manifest_option(mod_root: impl AsRef<Path>, option: ModOptionData) -> Result<ValidationResult> {
	let mod_root = mod_root.as_ref();

	match option {
		ModOptionData::Boolean { data, image, .. } => {
			if let Some(image) = image {
				match check_path_exists(mod_root, &*image)?.wrap_fail("Image is invalid") {
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}

			match validate_manifest_data(mod_root, data)? {
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}
		}

		ModOptionData::Selection {
			default_value, options, ..
		} => {
			if !options.iter().any(|opt| opt.id == default_value) {
				return fail!("Default selection value {default_value} does not exist");
			}

			if options.len() <= 1 {
				return fail!("Selection groups should have more than one option");
			}

			for option in options.0 {
				if let Some(image) = option.image {
					match check_path_exists(mod_root, &*image)?.wrap_fail("Image is invalid") {
						ValidationResult::Pass => {}
						failure @ ValidationResult::Fail(_) => return Ok(failure)
					}
				}

				match validate_manifest_data(mod_root, option.data)? {
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}
		}

		ModOptionData::Conditional { data, .. } => match validate_manifest_data(mod_root, data)? {
			ValidationResult::Pass => {}
			failure @ ValidationResult::Fail(_) => return Ok(failure)
		},

		ModOptionData::Number {
			ref image,
			default_value,
			..
		} => {
			match option
				.validate(&ModOptionValue::Number { value: default_value })
				.wrap_fail("Default value is invalid")
			{
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}

			if let Some(image) = image {
				match check_path_exists(mod_root, &**image)?.wrap_fail("Image is invalid") {
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}
		}

		ModOptionData::Color {
			ref image,
			ref default_value,
			..
		} => {
			match option
				.validate(&ModOptionValue::Color {
					value: default_value.to_owned()
				})
				.wrap_fail("Default value is invalid")
			{
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}

			if let Some(image) = image {
				match check_path_exists(mod_root, &**image)?.wrap_fail("Image is invalid") {
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}
		}

		ModOptionData::String {
			ref image,
			ref default_value,
			..
		} => {
			match option
				.validate(&ModOptionValue::String {
					value: default_value.to_owned()
				})
				.wrap_fail("Default value is invalid")
			{
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}

			if let Some(image) = image {
				match check_path_exists(mod_root, &**image)?.wrap_fail("Image is invalid") {
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}
		}

		ModOptionData::OptionGroup {
			options, presets, data, ..
		} => {
			match validate_manifest_data(mod_root, data)? {
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}

			for preset in presets {
				if let Some(image) = &preset.image {
					match check_path_exists(mod_root, &**image)?
						.wrap_fail("Image is invalid")
						.wrap_fail(format!(
							"Preset {} is invalid",
							preset.name.first_specified().expect("No localisation specified")
						)) {
						ValidationResult::Pass => {}
						failure @ ValidationResult::Fail(_) => return Ok(failure)
					}
				}

				for (option, value) in &preset.values {
					if let Some(opt) = ModOption::get_option_by_id(&options, option) {
						let typ = match &opt.data {
							ModOptionData::Boolean { .. } => "boolean",
							ModOptionData::Selection { .. } => "selection",
							ModOptionData::Number { .. } => "number",
							ModOptionData::Color { .. } => "color",
							ModOptionData::String { .. } => "string",
							ModOptionData::Conditional { .. } | ModOptionData::OptionGroup { .. } => {
								return Ok(ValidationResult::Fail(format!("Option {option} cannot be specified"))
									.wrap_fail(format!(
										"Preset {} is invalid",
										preset.name.first_specified().expect("No localisation specified")
									)));
							}
						};

						match opt
							.data
							.validate(&from_value(json!({
								"type": typ,
								"value": value
							}))?)
							.wrap_fail("Value for option {option} is invalid")
							.wrap_fail(format!(
								"Preset {} is invalid",
								preset.name.first_specified().expect("No localisation specified")
							)) {
							ValidationResult::Pass => {}
							failure @ ValidationResult::Fail(_) => return Ok(failure)
						}
					} else {
						return Ok(
							ValidationResult::Fail(format!("No such option {option} in group")).wrap_fail(format!(
								"Preset {} is invalid",
								preset.name.first_specified().expect("No localisation specified")
							))
						);
					}
				}
			}

			for option in options.0 {
				let option_id = option.id.to_owned();

				match validate_manifest_option(mod_root, option.data)?
					.wrap_fail(format!("Sub-option {option_id} is invalid"))
				{
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}
		}
	}

	ValidationResult::Pass
}

#[try_fn]
fn check_path_exists(mod_root: impl AsRef<Path>, path: impl AsRef<RelativePath>) -> Result<ValidationResult> {
	let mod_root = mod_root.as_ref();
	let path = path.as_ref();
	let resolved = path.to_logical_path(mod_root);

	if !resolved.exists() || fs::canonicalize(resolved)?.relative_to(fs::canonicalize(mod_root)?)? != path {
		return fail!("Path {} does not exist or has incorrect capitalisation", path);
	}

	ValidationResult::Pass
}

#[instrument(skip(mod_root), fields(mod_root = mod_root.as_ref().to_string_lossy().to_string()))]
#[try_fn]
pub fn validate_mod_folder(mod_root: impl AsRef<Path>, lenient: bool) -> Result<ValidationResult> {
	let mod_root = mod_root.as_ref();

	match check_path_exists(mod_root, "manifest.json")? {
		ValidationResult::Pass => {}
		failure @ ValidationResult::Fail(_) => return Ok(failure)
	}

	match serde_path_to_error::deserialize::<_, Manifest>(&mut Deserializer::from_slice(&fs::read(
		mod_root.join("manifest.json")
	)?)) {
		Ok(manifest) => {
			if !lenient && (manifest.version.major < 1 || manifest.url.is_none()) {
				return fail!("Mod is not ready for release (version or URL)");
			}

			if !lenient && manifest.id.split_once('.').unwrap().0 == "RPKGMod" {
				return fail!("Author cannot be RPKGMod");
			}

			if manifest.version.to_string() == H3_VERSION {
				return fail!("Manifest should contain mod version, not game version");
			}

			if manifest.conditions.supported_games.is_empty() {
				return fail!("Top-level manifest must specify at least one supported game");
			}

			match validate_manifest_data(mod_root, manifest.data)?.wrap_fail("Top-level manifest data is invalid") {
				ValidationResult::Pass => {}
				failure @ ValidationResult::Fail(_) => return Ok(failure)
			}

			let mut option_ids = vec![];

			for preset in &manifest.presets {
				option_ids.push(preset.id.to_owned());

				if let Some(image) = &preset.image {
					match check_path_exists(mod_root, &**image)?
						.wrap_fail("Image is invalid")
						.wrap_fail(format!(
							"Preset {} is invalid",
							preset.name.first_specified().expect("No localisation specified")
						)) {
						ValidationResult::Pass => {}
						failure @ ValidationResult::Fail(_) => return Ok(failure)
					}
				}

				for (option, value) in &preset.values {
					if let Some(opt) = ModOption::get_option_by_id(&manifest.options, option) {
						let typ = match &opt.data {
							ModOptionData::Boolean { .. } => "boolean",
							ModOptionData::Selection { .. } => "selection",
							ModOptionData::Number { .. } => "number",
							ModOptionData::Color { .. } => "color",
							ModOptionData::String { .. } => "string",
							ModOptionData::Conditional { .. } | ModOptionData::OptionGroup { .. } => {
								return Ok(ValidationResult::Fail(format!("Option {option} cannot be specified"))
									.wrap_fail(format!(
										"Preset {} is invalid",
										preset.name.first_specified().expect("No localisation specified")
									)));
							}
						};

						match opt
							.data
							.validate(&from_value(json!({
								"type": typ,
								"value": value
							}))?)
							.wrap_fail("Value for option {option} is invalid")
							.wrap_fail(format!(
								"Preset {} is invalid",
								preset.name.first_specified().expect("No localisation specified")
							)) {
							ValidationResult::Pass => {}
							failure @ ValidationResult::Fail(_) => return Ok(failure)
						}
					} else {
						return Ok(
							ValidationResult::Fail(format!("No such option {option}")).wrap_fail(format!(
								"Preset {} is invalid",
								preset.name.first_specified().expect("No localisation specified")
							))
						);
					}
				}
			}

			fn collect_option_ids(option: &ModOption, ids: &mut Vec<ModOptionID>) {
				ids.push(option.id.to_owned());

				if let ModOptionData::Selection { options, .. } = &option.data {
					for sub_option in options.0.iter() {
						ids.push(sub_option.id.to_owned());
					}
				}

				if let ModOptionData::OptionGroup { options, presets, .. } = &option.data {
					ids.extend(presets.iter().map(|x| x.id.to_owned()));

					for sub_option in options.0.iter() {
						collect_option_ids(sub_option, ids);
					}
				}
			}

			for option in manifest.options {
				collect_option_ids(&option, &mut option_ids);

				let option_id = option.id.to_owned();

				match validate_manifest_option(mod_root, option.data)?
					.wrap_fail(format!("Mod option {option_id} is invalid"))
				{
					ValidationResult::Pass => {}
					failure @ ValidationResult::Fail(_) => return Ok(failure)
				}
			}

			if option_ids.len() != option_ids.iter().collect::<HashSet<_>>().len() {
				return fail!("Mod contains duplicate option IDs");
			}

			ValidationResult::Pass
		}

		Err(err) => ValidationResult::Fail(format!("Manifest is invalid: {err}"))
	}
}
