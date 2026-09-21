use std::{
	fs,
	io::{Cursor, Read, Seek},
	path::Path,
	str::FromStr,
	sync::atomic::Ordering
};

use color_eyre::eyre::{OptionExt, Result, WrapErr, bail};
use fn_wrap_err::wrap_err;
use glacier_commons::{
	hash_list::HASH_LIST,
	metadata::{ResourceMetadata, RuntimeID},
	rpkg_tool::RpkgResourceMeta
};
use glacier_formats::texture::{RenderFormat, TextureMetadata, TextureType};
use image::ImageReader;
use rpkg_rs::resource::partition_manager::PartitionManager;
use tryvial::try_fn;

use crate::get_resource_partition;

#[try_fn]
#[wrap_err("Couldn't upgrade meta JSON {}", path.display())]
pub fn upgrade_meta_json(path: &Path) -> Result<()> {
	if HASH_LIST.version.load(Ordering::SeqCst) == 0 {
		HASH_LIST.load_cached().wrap_err("Couldn't load hash list")?;
	}

	let mut meta = ResourceMetadata::try_from(serde_json::from_slice::<RpkgResourceMeta>(&fs::read(path)?)?)?;

	if meta.compressed != ResourceMetadata::infer_compressed(meta.resource_type)
		|| meta.scrambled != ResourceMetadata::infer_scrambled(meta.resource_type)
		|| !meta.references.is_empty()
	{
		meta.id = path
			.file_name()
			.unwrap()
			.to_string_lossy()
			.split('.')
			.next()
			.unwrap()
			.parse::<RuntimeID>()?;

		fs::write(
			path.with_file_name(format!(
				"{}.{}.metadata.json",
				path.file_name().unwrap().to_string_lossy().split('.').next().unwrap(),
				path.file_name().unwrap().to_string_lossy().split('.').nth(1).unwrap()
			)),
			serde_json::to_vec(&meta)?
		)?;
	}

	fs::remove_file(path)?;
}

#[try_fn]
#[wrap_err("Couldn't upgrade binary meta {}", path.display())]
pub fn upgrade_binary_meta(path: &Path) -> Result<()> {
	if HASH_LIST.version.load(Ordering::SeqCst) == 0 {
		HASH_LIST.load_cached().wrap_err("Couldn't load hash list")?;
	}

	let mut meta = ResourceMetadata::try_from(RpkgResourceMeta::from_binary(&fs::read(path)?)?)?;

	if meta.compressed != ResourceMetadata::infer_compressed(meta.resource_type)
		|| meta.scrambled != ResourceMetadata::infer_scrambled(meta.resource_type)
		|| !meta.references.is_empty()
	{
		meta.id = path
			.file_name()
			.unwrap()
			.to_string_lossy()
			.split('.')
			.next()
			.unwrap()
			.parse::<RuntimeID>()?;

		fs::write(
			path.with_file_name(format!(
				"{}.{}.metadata.json",
				path.file_name().unwrap().to_string_lossy().split('.').next().unwrap(),
				path.file_name().unwrap().to_string_lossy().split('.').nth(1).unwrap()
			)),
			serde_json::to_vec(&meta)?
		)?;
	}

	fs::remove_file(path)?;
}

#[try_fn]
#[wrap_err("Couldn't upgrade raw file {}", path.display())]
pub fn upgrade_raw_file(game_files: &PartitionManager, path: &Path) -> Result<()> {
	let filename = path.file_name().unwrap().to_string_lossy();
	let mut filetype = filename.split(".").skip(1).collect::<Vec<_>>().join(".");

	// Skip files that aren't actual raw resources
	if filetype.len() == 4 && filetype != "meta" {
		let mut path = path.to_owned();

		// Fix casing
		if filename != filename.to_uppercase() {
			fs::rename(&path, path.with_file_name(filename.to_uppercase()))?;

			if path.with_added_extension("metadata.json").exists() {
				fs::rename(
					path.with_added_extension("metadata.json"),
					path.with_file_name(filename.to_uppercase())
						.with_added_extension("metadata.json")
				)?;
			}

			path = path.with_file_name(filename.to_uppercase());
			filetype = filetype.to_uppercase();
		}

		if path.with_added_extension("meta").exists() {
			upgrade_binary_meta(&path.with_added_extension("meta"))?;
		}

		if path.with_added_extension("meta.json").exists() {
			upgrade_meta_json(&path.with_added_extension("meta.json"))?;
		}

		let id = path
			.file_stem()
			.ok_or_eyre("No file stem")?
			.to_string_lossy()
			.parse::<RuntimeID>()
			.wrap_err("Invalid filename for raw file")?;

		let metadata = if path.with_added_extension("metadata.json").exists() {
			serde_json::from_slice::<ResourceMetadata>(&fs::read(path.with_added_extension("metadata.json"))?)?
		} else {
			ResourceMetadata {
				id,
				resource_type: filetype.parse()?,
				compressed: ResourceMetadata::infer_compressed(filetype.parse()?),
				scrambled: ResourceMetadata::infer_scrambled(filetype.parse()?),
				references: vec![]
			}
		};

		if let Some(partition) = get_resource_partition(game_files, id)? {
			let partition = game_files
				.partitions
				.iter()
				.find(|p| p.partition_info().name.as_ref() == Some(partition))
				.ok_or_eyre("No such partition")?;

			let res_meta = ResourceMetadata::try_from(partition.get_resource_info(&id.as_u64().into())?)?;

			if metadata == res_meta {
				// Might be same as vanilla, check actual contents
				if partition.read_resource(&id.as_u64().into())? == fs::read(&path)? {
					// Remove if identical to vanilla file
					fs::remove_file(&path)?;

					// Remove meta as well
					if path.with_added_extension("metadata.json").exists() {
						fs::remove_file(path.with_added_extension("metadata.json"))?;
					}
				}
			}
		}
	}
}

#[try_fn]
#[wrap_err("Couldn't upgrade texture meta {}", path.display())]
pub fn upgrade_texture_meta(path: &Path) -> Result<()> {
	if HASH_LIST.version.load(Ordering::SeqCst) == 0 {
		HASH_LIST.load_cached().wrap_err("Couldn't load hash list")?;
	}

	let mut data = Cursor::new(fs::read(path)?);

	let ty = u16::from_le_bytes({
		let mut x = [0u8; 2];
		data.read_exact(&mut x)?;
		x
	});

	// Ignored (4 bytes of flags and 2 bytes of padding)
	data.seek_relative(6)?;

	let format = u16::from_le_bytes({
		let mut x = [0u8; 2];
		data.read_exact(&mut x)?;
		x
	});

	let text = RuntimeID::from_str(
		path.file_name()
			.unwrap()
			.to_string_lossy()
			.split('.')
			.next()
			.unwrap()
			.split('~')
			.next()
			.unwrap()
	)?;

	let meta = TextureMetadata {
		text,
		texd: path
			.file_name()
			.unwrap()
			.to_string_lossy()
			.split('.')
			.next()
			.unwrap()
			.split('~')
			.nth(1)
			.map(RuntimeID::from_str)
			.transpose()?,
		texture_type: match ty {
			0 => TextureType::Colour,
			1 => TextureType::Normal,
			2 => TextureType::Height,
			3 => TextureType::CompoundNormal,
			4 => TextureType::Billboard,
			_ => bail!("Unknown type {}", ty)
		},
		format: match format {
			0x0A => RenderFormat::R16G16B16A16,
			0x1C => RenderFormat::R8G8B8A8,
			0x34 => RenderFormat::R8G8,
			0x42 => RenderFormat::A8,
			0x49 => RenderFormat::BC1,
			0x4C => RenderFormat::BC2,
			0x4F => RenderFormat::BC3,
			0x52 => RenderFormat::BC4,
			0x55 => RenderFormat::BC5,
			0x5A => RenderFormat::BC7,
			_ => bail!("Unknown format {}", format)
		},
		interpret_as: Default::default()
	};

	let new_path = path.with_file_name(format!(
		"{}.texture.json",
		path.file_name().unwrap().to_string_lossy().split('.').next().unwrap()
	));

	fs::write(&new_path, serde_json::to_vec(&meta)?)?;
	fs::remove_file(path)?;

	let tga_path = new_path.with_extension("tga");
	ImageReader::open(&tga_path)?
		.decode()?
		.save(new_path.with_extension("png"))?;
	fs::remove_file(tga_path)?;
}
