use std::{
	fmt::Debug,
	fs,
	ops::Deref,
	path::{Path, PathBuf},
	sync::Arc
};

use color_eyre::{
	Section,
	eyre::{OptionExt, Result, WrapErr, bail, eyre}
};
use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_base::encryption::xtea::{Xtea, XteaConfig};
use glacier_commons::{
	game::{GamePlatform, GlacierGame},
	metadata::{ResourceMetadata, RuntimeID},
	rid
};
use glacier_ini::IniFileSystem;
use identity_hash::BuildIdentityHasher;
use itertools::Itertools;
use lazy_regex::{regex_captures, regex_captures_iter};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use relative_path::{PathExt, RelativePath, RelativePathBuf};
use rpkg_rs::resource::{
	partition_manager::{PartitionManager, PartitionManagerPar},
	pdefs::{
		PackageDefinitionParser, PackageDefinitionSource, PartitionId, PartitionType, bond_parser::BondParser,
		h2016_parser::H2016Parser, hm2_parser::HM2Parser, hm3_parser::HM3Parser
	},
	resource_partition::PatchId,
	runtime_resource_id::RuntimeResourceID
};
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use simple_mod_framework_types::{Game, HashMap, Manifest, ModID, PapayaMap, PapayaSet, Platform, VersionPlatform};
use specta::Type;
use tracing::instrument;
use tryvial::try_fn;
use walkdir::WalkDir;

use crate::{
	CLEAN_PACKAGE_DEFINITION, FL_VERSION, GAME_HASHES, H3_VERSION,
	diagnostics::{Diagnostic, DiagnosticKind, DiagnosticTarget},
	intentional_halt,
	rkyv_helpers::EcoStringAsBytes,
	utils::{ResultExt, format_json},
	world::{Diagnostics, Mods, ModsWritable, Output}
};

pub const REPO_ID: RuntimeID = rid!("[assembly:/repository/pro.repo].pc_repo");

pub const UNLOCKABLES_ID_WOA: RuntimeID =
	rid!("[assembly:/_pro/online/default/offlineconfig/config.unlockables].pc_unlockables");

pub const UNLOCKABLES_ID_FL: RuntimeID =
	rid!("[assembly:/_knt/online/default/offlineconfig/config.unlockables].unlockables");

/// Information and loaded files of a game instance.
/// Immutable and can be reused between deployments.
pub struct GameContext {
	pub version: GlacierGame,

	pub platform: Platform,

	/// An MD5 hash of the game executable (or game config, if Microsoft).
	pub hash: String,

	/// The Retail folder.
	pub retail_path: PathBuf,

	/// The Runtime folder.
	pub runtime_path: PathBuf,

	/// The base game `packagedefinition.txt` contents.
	pub clean_package_definition: String,

	/// The `rpkg-rs` partition manager.
	pub game_files: PartitionManager,

	/// Sorted by the order of the partition in the game files.
	pub vanilla_resources: HashMap<RuntimeID, Vec<RealPartition>, BuildIdentityHasher<u64>>,

	/// Sorted by the order of the partition in the game files.
	pub deleted_resources: HashMap<RuntimeID, Vec<RealPartition>, BuildIdentityHasher<u64>>,

	pub accessible_partitions: PapayaMap<EcoString, Vec<RealPartition>>
}

impl GameContext {
	#[try_fn]
	pub fn from_partition_manager(
		game: Game,
		runtime_path: PathBuf,
		clean_package_definition: String,
		game_files: PartitionManager
	) -> Result<Self> {
		if let Some(x) = game_files.partitions.iter().find(|p| p.partition_info().name.is_none()) {
			intentional_halt!(format!(
				"Partition {} has no name. Verify your game files, and if this keeps happening, report the issue.",
				x.partition_info().id
			));
		}

		let (vanilla_resources, deleted_resources) = rayon::join(
			|| {
				let _span = tracing::info_span!("Caching vanilla resources").entered();
				let mut res = HashMap::default();
				for (key, val) in game_files
					.partitions
					.par_iter()
					.flat_map_iter(|partition| {
						let partition_name = RealPartition(partition.partition_info().name.as_deref().unwrap().into());

						partition
							.latest_resources()
							.into_iter()
							.map(move |(info, _)| Ok(((*info.rrid()).try_into()?, partition_name.to_owned())))
					})
					.collect::<Result<Vec<_>>>()?
				{
					res.entry(key).or_insert_with(Vec::new).push(val);
				}

				color_eyre::eyre::Ok(res)
			},
			|| {
				let _span = tracing::info_span!("Caching deleted resources").entered();
				let mut res = HashMap::default();
				for (key, val) in game_files
					.partitions
					.par_iter()
					.flat_map_iter(|partition| {
						let partition_name = RealPartition(partition.partition_info().name.as_deref().unwrap().into());

						partition
							.removed_resources()
							.into_iter()
							.map(move |(info, _)| Ok(((*info.rrid()).try_into()?, partition_name.to_owned())))
					})
					.collect::<Result<Vec<_>>>()?
				{
					res.entry(key).or_insert_with(Vec::new).push(val);
				}

				color_eyre::eyre::Ok(res)
			}
		);

		let (vanilla_resources, deleted_resources) = (vanilla_resources?, deleted_resources?);

		Self {
			version: game.version,
			platform: game.platform,
			hash: game.hash,
			retail_path: game.path,
			runtime_path,
			clean_package_definition,
			game_files,
			vanilla_resources,
			deleted_resources,
			accessible_partitions: Default::default()
		}
	}

	pub fn from_game(game: Game, world: &impl Diagnostics, clear_mods: bool) -> Result<Self> {
		let (runtime_path, clean_package_definition, game_files) =
			load_game_files(world, &game.path, game.version, clear_mods)?;

		Self::from_partition_manager(game, runtime_path, clean_package_definition, game_files)
	}

	#[try_fn]
	pub fn get_accessible_partitions(&self, from: &str) -> Result<Vec<RealPartition>> {
		let guard = self.accessible_partitions.pin();

		if let Some(cached) = guard.get(from) {
			cached.to_owned()
		} else {
			let mut partition = self
				.game_files
				.partitions
				.iter()
				.find(|x| x.partition_info().name.as_deref().unwrap() == from)
				.ok_or_else(|| eyre!("No such partition {from}"));

			if self.version != GlacierGame::H3
				&& from != *NominalPartition("super".into()).real_candidates(self).unwrap()[0]
			{
				partition = partition.suggestion("is the game DLC for this location installed?");
			}

			let mut partition = partition?;

			let mut accessible_partitions = vec![RealPartition(
				partition.partition_info().name.as_deref().unwrap().into()
			)];

			while let Some(parent) = partition.partition_info().parent.as_ref() {
				partition = self
					.game_files
					.partitions
					.iter()
					.find(|x| x.partition_info().id == *parent)
					.ok_or_eyre("No such parent partition")?;

				accessible_partitions.push(RealPartition(
					partition.partition_info().name.as_deref().unwrap().into()
				));
			}

			guard.insert(from.into(), accessible_partitions.to_owned());
			accessible_partitions
		}
	}

	/// Construct a ResourceSpecifier from only a resource's hash by automatically finding the correct partition.
	#[try_fn]
	#[wrap_err("Couldn't infer resource specifier from ID")]
	pub fn infer_resource_specifier(&self, id: RuntimeID) -> Result<Option<ResourceSpecifier>> {
		if let Some(partitions) = self.vanilla_resources.get(&id)
			&& let Some(partition) = partitions.first()
		{
			Some(ResourceSpecifier {
				id,
				partition: partition.to_owned()
			})
		} else {
			None
		}
	}

	pub fn to_rrid_u64(&self, id: RuntimeID) -> u64 {
		if id.is_agnostic() {
			id.as_u64() | ((GamePlatform::PC.tag().unwrap() as u64) << 56)
		} else {
			id.as_u64()
		}
	}

	pub fn to_rrid(&self, id: RuntimeID) -> RuntimeResourceID {
		self.to_rrid_u64(id).into()
	}

	/// Construct a ResourceSpecifier from a resource's hash and its nominal partition. Will return None if and only if the partition has no real equivalent in the currently deploying game.
	#[wrap_err("Couldn't infer resource specifier from ID and nominal partition")]
	pub fn realise_resource_specifier(
		&self,
		id: RuntimeID,
		partition: NominalPartition
	) -> Result<Option<ResourceSpecifier>> {
		if let Some(candidates) = partition.real_candidates(self) {
			for candidate in &candidates {
				let partition = self
					.game_files
					.partitions
					.iter()
					.find(|x| x.partition_info().name.as_deref().unwrap() == candidate.as_str());

				if let Some(partition) = partition
					&& partition.contains(&self.to_rrid(id))
				{
					return Ok(Some(ResourceSpecifier {
						id,
						partition: candidate.to_owned()
					}));
				}
			}

			// Fall back to first candidate (e.g. new file being added which is not yet in the game files)
			Ok(Some(ResourceSpecifier {
				id,
				partition: candidates
					.first()
					.ok_or_eyre("No real candidates for nominal partition")?
					.to_owned()
			}))
		} else {
			Ok(None)
		}
	}

	#[try_fn]
	pub fn get_metadata(&self, spec: &ResourceSpecifier) -> Option<(u32, ResourceMetadata)> {
		let partition = self
			.game_files
			.partitions
			.iter()
			.find(|x| x.partition_info().name.as_deref() == Some(spec.partition.as_str()))?;

		let res_info = partition.get_resource_info(&self.to_rrid(spec.id)).ok()?;

		(
			res_info.size(),
			res_info.try_into().expect("rpkg-rs returned invalid resource info")
		)
	}

	#[try_fn]
	pub fn get_data(&self, spec: &ResourceSpecifier) -> Result<Vec<u8>> {
		let partition = self
			.game_files
			.partitions
			.iter()
			.find(|x| x.partition_info().name.as_deref() == Some(spec.partition.as_str()))
			.ok_or_eyre("No such partition")?;

		partition
			.read_resource(&self.to_rrid(spec.id))
			.context("Couldn't extract resource using rpkg-rs")?
	}
}

impl From<&GameContext> for VersionPlatform {
	fn from(value: &GameContext) -> Self {
		VersionPlatform {
			version: value.version,
			platform: value.platform
		}
	}
}

#[try_fn]
#[instrument(skip_all)]
pub fn detect_game(path: impl Into<PathBuf>) -> Result<Option<Game>> {
	let path = path.into();

	if path.join("HITMAN3.exe").exists() {
		let hash = format!(
			"{:x}",
			md5::compute(if path.join("Runtime").join("chunk0.rpkg").exists() {
				fs::read(path.join("../../..").join("MicrosoftGame.Config")).wrap_err("Couldn't read game config")?
			} else {
				fs::read(path.join("HITMAN3.exe")).wrap_err("Couldn't read game EXE")?
			})
		);

		let Some(platform) = GAME_HASHES.get(&*hash) else {
			intentional_halt!(format!(
				"Unrecognised game version. This version of SMF is designed for version {H3_VERSION} of HITMAN 3. If \
				 the game has recently updated, the framework will need to be patched by its developers. If you're \
				 using a pirated or cracked version of the game, that's the problem."
			));
		};

		Some(Game {
			version: platform.version,
			platform: platform.platform,
			hash,
			path
		})
	} else if path.join("007FirstLight.exe").exists() {
		let hash = format!(
			"{:x}",
			md5::compute(fs::read(path.join("007FirstLight.exe")).wrap_err("Couldn't read game EXE")?)
		);

		let Some(platform) = GAME_HASHES.get(&*hash) else {
			intentional_halt!(format!(
				"Unrecognised game version. This version of SMF is designed for version {FL_VERSION} of 007 First \
				 Light. If the game has recently updated, the framework will need to be patched by its developers. If \
				 you're using a pirated or cracked version of the game, that's the problem."
			));
		};

		Some(Game {
			version: platform.version,
			platform: platform.platform,
			hash,
			path
		})
	} else if path.join("HITMAN2.exe").exists() {
		Some(Game {
			version: GlacierGame::H2,
			platform: if path.join("steam_api64.dll").exists() {
				Platform::Steam
			} else {
				Platform::Epic
			},
			hash: format!(
				"{:x}",
				md5::compute(fs::read(path.join("HITMAN2.exe")).wrap_err("Couldn't read game EXE")?)
			),
			path
		})
	} else if path.join("HITMAN.exe").exists() {
		Some(Game {
			version: GlacierGame::H1,
			platform: if path.join("steam_api64.dll").exists() {
				Platform::Steam
			} else if path.join("EOSSDK-Win64-Shipping.dll").exists() {
				Platform::Epic
			} else if path.join("../../..").join("MicrosoftGame.Config").exists() {
				Platform::Microsoft
			} else {
				Platform::GOG
			},
			hash: format!(
				"{:x}",
				md5::compute(if path.join("../../..").join("MicrosoftGame.Config").exists() {
					fs::read(path.join("../../..").join("MicrosoftGame.Config"))
						.wrap_err("Couldn't read game config")?
				} else {
					fs::read(path.join("HITMAN.exe")).wrap_err("Couldn't read game EXE")?
				})
			),
			path
		})
	} else {
		None
	}
}

#[try_fn]
#[wrap_err("Couldn't load game files")]
#[instrument(skip_all)]
pub fn load_game_files(
	world: &impl Diagnostics,
	game_path: impl AsRef<Path>,
	game_version: GlacierGame,
	clear_mods: bool
) -> Result<(PathBuf, String, PartitionManager)> {
	let game_path = game_path.as_ref();

	let xtea_config = if game_version == GlacierGame::FL {
		XteaConfig::KNT
	} else {
		XteaConfig::Woa
	};

	let xtea = Xtea::new(xtea_config);

	let thumbs =
		IniFileSystem::from_path(game_path.join("thumbs.dat"), xtea_config).wrap_err("Couldn't load thumbs.dat")?;

	let (Ok(proj_path), Ok(relative_runtime_path)) = (
		thumbs.option(("application", "PROJECT_PATH")),
		thumbs.option(("application", "RUNTIME_PATH"))
	) else {
		bail!("thumbs.dat was missing required properties");
	};

	// Workaround for the Linux filesystem's case sensitivity. Copied from GlacierKit (credit: Dafitius)
	let relative_runtime_path_uppercased = relative_runtime_path
		.char_indices()
		.map(|(idx, ch)| if idx == 0 { ch.to_ascii_uppercase() } else { ch })
		.collect::<String>();

	let runtime_folder = [relative_runtime_path, relative_runtime_path_uppercased]
		.iter()
		.flat_map(|folder| game_path.join(proj_path.replace('\\', "/")).join(folder).canonicalize())
		.find(|joined_path| joined_path.exists())
		.ok_or_eyre("Couldn't find valid runtime folder")?;

	let package_definition = if game_version == GlacierGame::H3 {
		CLEAN_PACKAGE_DEFINITION.to_owned()
	} else {
		let existing_pdef = fs::read(runtime_folder.join("packagedefinition.txt"))
			.wrap_err("Couldn't read packagedefinition.txt file")
			.intentional()?;

		let existing_pdef = if xtea.is_encrypted_text_file(&existing_pdef) {
			xtea.decrypt_text_file(&existing_pdef)
				.map_err(|x| eyre!("XTEA decryption error for thumbs: {:?}", x))?
		} else {
			String::from_utf8(existing_pdef)?
		};

		if regex_captures_iter!(r"patchlevel=(\d+)", &existing_pdef).any(|x| x[1].parse::<usize>().is_ok_and(|x| x > 9))
			|| existing_pdef.contains("Patched by Simple Mod Framework.")
		{
			if !runtime_folder.join("packagedefinition.txt.clean").exists() {
				intentional_halt!(
					"Your packagedefinition.txt is modded and no clean copy can be found! Verify your game files and \
					 try again."
				);
			}

			fs::read_to_string(runtime_folder.join("packagedefinition.txt.clean"))
				.wrap_err("Couldn't read packagedefinition.txt.clean file")
				.intentional()?
		} else {
			if clear_mods {
				fs::write(runtime_folder.join("packagedefinition.txt.clean"), &existing_pdef)?;
			}

			existing_pdef
		}
	};

	let mut partitions = match game_version {
		GlacierGame::H1 => H2016Parser::parse(package_definition.as_bytes()),
		GlacierGame::H2 => HM2Parser::parse(package_definition.as_bytes()),
		GlacierGame::H3 => HM3Parser::parse(package_definition.as_bytes()),
		GlacierGame::FL => BondParser::parse(package_definition.as_bytes())
	}
	.wrap_err("Couldn't read packagedefinition")?;

	for partition in &mut partitions {
		partition.set_max_patch_level(9);
	}

	let partition_names = partitions
		.iter()
		.filter_map(|x| x.name.as_ref().map(|name| (x.id.index, name.to_owned())))
		.collect::<HashMap<_, _>>();

	// Give names to unnamed language partitions (fixes H1)
	for partition in &mut partitions {
		if partition.name.is_none() {
			match &partition.id.part_type {
				PartitionType::LanguageStandard(lang) | PartitionType::LanguageDlc(lang) => {
					partition.name = Some(format!(
						"{}-{}",
						partition_names
							.get(&partition.id.index)
							.ok_or_eyre("Language package had no associated regular package")?,
						lang
					));
				}

				_ => {}
			}
		}
	}

	for rpkg in fs::read_dir(&runtime_folder).wrap_err("Couldn't read runtime folder")? {
		let rpkg = rpkg?;
		if let Some((_, patch)) = regex_captures!(r"patch([0-9]+).rpkg", &rpkg.file_name().to_string_lossy()) {
			let patch = patch.parse::<usize>()?;

			if let Some(partition) = partitions
				.iter()
				.find(|x| x.filename(PatchId::Patch(patch)) == rpkg.file_name().to_string_lossy())
			{
				if patch > partition.patch_level {
					if patch == 100
						|| patch == 300 || (game_version == GlacierGame::H1 && patch == partition.patch_level + 1)
					{
						if clear_mods {
							fs::remove_file(rpkg.path())?;
						}
					} else {
						world.emit_diagnostic(Diagnostic {
							kind: DiagnosticKind::UnrecognizedPatch {
								file_name: rpkg.file_name().to_string_lossy().to_string()
							},
							target: DiagnosticTarget::None
						})?;
					}
				}
			} else {
				world.emit_diagnostic(Diagnostic {
					kind: DiagnosticKind::UnrecognizedPartitionPatch {
						file_name: rpkg.file_name().to_string_lossy().to_string()
					},
					target: DiagnosticTarget::None
				})?;
			}
		}
	}

	if game_path.join("mods").exists() {
		for file in fs::read_dir(game_path.join("mods"))
			.wrap_err("Couldn't read SDK mods folder")?
			.filter_map(|entry| entry.ok())
		{
			if file.file_type()?.is_symlink() && clear_mods {
				fs::remove_file(file.path()).wrap_err("Couldn't remove symlink from SDK mods folder")?;
			}
		}
	}

	let mut partition_manager = PartitionManager::new(
		runtime_folder.to_owned(),
		game_version.into(),
		&PackageDefinitionSource::Custom(partitions)
	)
	.wrap_err("Couldn't create partition manager")?;

	partition_manager
		.mount_partitions_par(|_, _| {})
		.wrap_err("Couldn't mount partitions")?;

	(runtime_folder, package_definition, partition_manager)
}

/// The name of a partition in HITMAN 3.
#[derive(
	PartialOrd,
	Ord,
	Debug,
	Hash,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
	Clone,
	better_rune_derive::Any,
	rkyv::Archive,
	rkyv::Serialize,
	rkyv::Deserialize,
)]
#[repr(transparent)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE, PARTIAL_EQ, EQ)]
#[rune(constructor)]
#[rune_functions(Self::r_real_candidates)]
pub struct NominalPartition(
	#[rkyv(with = EcoStringAsBytes)]
	#[rune(get, set, as_into = String)]
	pub EcoString
);

impl Deref for NominalPartition {
	type Target = EcoString;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl NominalPartition {
	#[rune::function(instance, path = Self::real_candidates)]
	fn r_real_candidates(&self, platform: VersionPlatform) -> Option<Vec<RealPartition>> {
		self.real_candidates(platform)
	}

	pub fn real_candidates(&self, platform: impl Into<VersionPlatform>) -> Option<Vec<RealPartition>> {
		let platform = platform.into();

		match (platform.version, platform.platform) {
			(GlacierGame::H1, Platform::Epic) => match &self.0 {
				x if [
					"season3",
					"ancestral",
					"edgy",
					"elegant",
					"wet",
					"trapped",
					"golden",
					"season2",
					"opulent",
					"caged",
					"greedy",
					"salty",
					"hawk",
					"theark",
					"skunk",
					"mongoose",
					"colombia",
					"miami",
					"sheep"
				]
				.contains(&x.as_str()) =>
				{
					None
				}

				x if [
					"super",
					"base",
					"legacy",
					"season1",
					"hokkaido",
					"colorado",
					"bangkok",
					"marrakesh",
					"coastaltown",
					"paris"
				]
				.contains(&x.as_str()) =>
				{
					Some(vec![RealPartition("Base Game & Polar Bear".into())])
				}

				// On Epic there are no other partitions, so just override it to Base Game & Polar Bear
				x if x.starts_with("h1-") => Some(vec![RealPartition("Base Game & Polar Bear".into())]),

				x if x.starts_with("h2-") => None,
				x if x.starts_with("h3-") => None,
				x if x.starts_with("fl-") => None,

				x => Some(vec![RealPartition(x.to_owned())])
			},

			(GlacierGame::H1, _) => match &self.0 {
				x if x == "super" || x == "base" || x == "legacy" || x == "season1" => {
					Some(vec![RealPartition("Base Game & Polar Bear".into())])
				}

				x if [
					"season3",
					"ancestral",
					"edgy",
					"elegant",
					"wet",
					"trapped",
					"golden",
					"season2",
					"opulent",
					"caged",
					"greedy",
					"salty",
					"hawk",
					"theark",
					"skunk",
					"mongoose",
					"colombia",
					"miami",
					"sheep"
				]
				.contains(&x.as_str()) =>
				{
					None
				}

				x if x == "hokkaido" => Some(vec![RealPartition("Snow Crane".into())]),
				x if x == "colorado" => Some(vec![RealPartition("Bull".into())]),
				x if x == "bangkok" => Some(vec![RealPartition("Tiger".into())]),
				x if x == "marrakesh" => Some(vec![
					RealPartition("Spider".into()),
					RealPartition("Copperhead & Python (& Mamba)".into()),
				]),
				x if x == "coastaltown" => Some(vec![
					RealPartition("Octopus".into()),
					RealPartition("Copperhead & Python (& Mamba)".into()),
				]),
				x if x == "paris" => Some(vec![RealPartition("Peacock".into())]),

				x if x.starts_with("h1-") => Some(vec![RealPartition(x.trim_start_matches("h1-").into())]),
				x if x.starts_with("h2-") => None,
				x if x.starts_with("h3-") => None,
				x if x.starts_with("fl-") => None,

				x => Some(vec![RealPartition(x.to_owned())])
			},

			(GlacierGame::H2, _) => match &self.0 {
				x if x == "super" || x == "base" || x == "legacy" || x == "season1" || x == "season2" => {
					Some(vec![RealPartition("Base Game & Polar Bear".into())])
				}

				x if ["season3", "ancestral", "edgy", "elegant", "wet", "trapped", "golden"].contains(&x.as_str()) => {
					None
				}

				x if x == "opulent" => Some(vec![RealPartition("Stingray (Opulent)".into())]),
				x if x == "caged" => Some(vec![RealPartition("Falcon (Caged)".into())]),
				x if x == "greedy" => Some(vec![RealPartition("Raccoon (Greedy)".into())]),
				x if x == "salty" => Some(vec![RealPartition("Seagull (Salty)".into())]),
				x if x == "hawk" => Some(vec![RealPartition("Hawk (Himmelstein)".into())]),
				x if x == "theark" => Some(vec![RealPartition("Magpie (Isle of Sgàil)".into())]),
				x if x == "skunk" => Some(vec![
					RealPartition("Skunk (Whittleton Creek)".into()),
					RealPartition("Cottonmouth & GarterSnake (Flamingo/Skunk)".into()),
				]),
				x if x == "mongoose" => Some(vec![
					RealPartition("Mongoose (Mumbai)".into()),
					RealPartition("Anaconda & KingCobra (Hippo/Mongoose)".into()),
				]),
				x if x == "colombia" => Some(vec![
					RealPartition("Hippo (Santa Fortuna)".into()),
					RealPartition("Anaconda & KingCobra (Hippo/Mongoose)".into()),
				]),
				x if x == "miami" => Some(vec![
					RealPartition("Flamingo (Miami)".into()),
					RealPartition("Cottonmouth & GarterSnake (Flamingo/Skunk)".into()),
				]),

				x if x == "sheep" => Some(vec![
					RealPartition("Sheep (content in base)".into()),
					RealPartition("Base Game & Polar Bear".into()),
				]),

				x if x == "hokkaido" => Some(vec![RealPartition("Snow Crane (Hokkaido)".into())]),
				x if x == "colorado" => Some(vec![RealPartition("Bull (Colorado)".into())]),
				x if x == "bangkok" => Some(vec![RealPartition("Tiger (Bangkok)".into())]),
				x if x == "marrakesh" => Some(vec![
					RealPartition("Spider (Marrakech)".into()),
					RealPartition("Copperhead, Python & Mamba (Sapienza/Marrakesh)".into()),
				]),
				x if x == "coastaltown" => Some(vec![
					RealPartition("Octopus (Sapienza)".into()),
					RealPartition("Copperhead, Python & Mamba (Sapienza/Marrakesh)".into()),
				]),
				x if x == "paris" => Some(vec![RealPartition("Peacock (Paris)".into())]),

				x if x.starts_with("h1-") => None,
				x if x.starts_with("h2-") => Some(vec![RealPartition(x.trim_start_matches("h2-").into())]),
				x if x.starts_with("h3-") => None,
				x if x.starts_with("fl-") => None,

				x => Some(vec![RealPartition(x.to_owned())])
			},

			(GlacierGame::H3, _) => match &self.0 {
				x if x.starts_with("h1-") => None,
				x if x.starts_with("h2-") => None,
				x if x.starts_with("h3-") => Some(vec![RealPartition(x.trim_start_matches("h3-").into())]),
				x if x.starts_with("fl-") => None,

				x => Some(vec![RealPartition(x.to_owned())])
			},

			(GlacierGame::FL, _) => match &self.0 {
				x if x.starts_with("h1-") => None,
				x if x.starts_with("h2-") => None,
				x if x.starts_with("h3-") => None,
				x if x.starts_with("fl-") => Some(vec![RealPartition(x.trim_start_matches("fl-").into())]),

				x => Some(vec![RealPartition(x.to_owned())])
			}
		}
	}
}

/// The name of a partition in the currently deploying game.
#[derive(
	PartialOrd,
	Ord,
	Debug,
	Hash,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
	Clone,
	Type,
	better_rune_derive::Any,
	rkyv::Archive,
	rkyv::Serialize,
	rkyv::Deserialize,
)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE, PARTIAL_EQ, EQ)]
#[repr(transparent)]
pub struct RealPartition(
	#[rkyv(with = EcoStringAsBytes)]
	#[rune(get, set, as_into = String)]
	#[specta(type = String)]
	pub EcoString
);

impl Deref for RealPartition {
	type Target = EcoString;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// Represents a specific version of a specific game resource, by its RuntimeID and partition.
#[derive(
	PartialOrd,
	Ord,
	Hash,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
	Clone,
	Type,
	better_rune_derive::Any,
	rkyv::Archive,
	rkyv::Serialize,
	rkyv::Deserialize,
)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, CLONE, PARTIAL_EQ, EQ)]
#[rune(constructor)]
pub struct ResourceSpecifier {
	#[rune(get, set)]
	pub id: RuntimeID,

	#[rune(get, set)]
	pub partition: RealPartition
}

impl Debug for ResourceSpecifier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} in {}", self.id, self.partition.0)
	}
}

#[macro_export]
macro_rules! resource_in {
	($id:expr, $partition:literal, $game:expr) => {
		$crate::game::ResourceSpecifier {
			id: ::glacier_commons::rid!($id),
			partition: $crate::game::NominalPartition($partition.into())
				.real_candidates($game)
				.unwrap()[0]
				.to_owned()
		}
	};
}

#[derive(Debug, Clone)]
pub struct GameOutput {
	is_custom: bool,
	retail_path: PathBuf,
	dx12_retail_path: Option<PathBuf>,
	runtime_path: PathBuf,
	version: GlacierGame
}

impl GameOutput {
	pub fn from_game(game: &GameContext) -> Self {
		Self {
			is_custom: false,
			retail_path: game.retail_path.to_owned(),
			dx12_retail_path: {
				let path = game.retail_path.parent().unwrap().join("dx12Retail");
				path.exists().then_some(path)
			},
			runtime_path: game.runtime_path.to_owned(),
			version: game.version
		}
	}

	pub fn from_custom(game: &GameContext, output_folder: impl AsRef<Path>) -> Self {
		let output_folder = output_folder.as_ref();

		{
			let _span = tracing::info_span!("Clearing custom output directory").entered();
			let _ = fs::remove_dir_all(output_folder);
		}

		Self {
			is_custom: true,
			retail_path: output_folder.join("Retail"),
			dx12_retail_path: game
				.retail_path
				.parent()
				.unwrap()
				.join("dx12Retail")
				.exists()
				.then_some(output_folder.join("dx12Retail")),
			runtime_path: output_folder.join("Runtime"),
			version: game.version
		}
	}
}

impl Output for GameOutput {
	#[try_fn]
	fn emit_sdk_mod(&self, name: String, path: PathBuf) -> Result<()> {
		if !self.is_custom
			&& (!(self.retail_path.join("ZHMModSDK.dll").exists() || self.retail_path.join("ZKntSdk.dll").exists())
				|| !self.retail_path.join("dinput8.dll").exists())
		{
			intentional_halt!(
				"Deploying SDK mods requires the appropriate SDK to be installed. The Mod Manager can do this for \
				 you; check the Settings page."
			);
		}

		fs::create_dir_all(self.retail_path.join("mods"))?;

		let sdk_dll = self.retail_path.join("mods").join(name);

		#[cfg(windows)]
		{
			std::os::windows::fs::symlink_file(&path, sdk_dll)
				.wrap_err("Couldn't create symlink in SDK mods folder")?;
		}
		#[cfg(unix)]
		{
			std::os::unix::fs::symlink(&path, sdk_dll).wrap_err("Couldn't create symlink in SDK mods folder")?;
		}
	}

	#[try_fn]
	fn emit_package_definition(&self, package_definition: String) -> Result<()> {
		fs::create_dir_all(&self.runtime_path)?;
		fs::write(
			self.runtime_path.join("packagedefinition.txt"),
			Xtea::new(if self.version == GlacierGame::FL {
				XteaConfig::KNT
			} else {
				XteaConfig::Woa
			})
			.encrypt_text_file(package_definition.to_owned())
			.map_err(|x| eyre!("XTEA encryption error: {:?}", x))?
		)?;
	}

	#[try_fn]
	fn emit_thumbs(&self, thumbs: String) -> Result<()> {
		if let Some(dx12_retail_path) = &self.dx12_retail_path {
			fs::create_dir_all(dx12_retail_path)?;
			fs::write(
				dx12_retail_path.join("thumbs.dat"),
				Xtea::new(if self.version == GlacierGame::FL {
					XteaConfig::KNT
				} else {
					XteaConfig::Woa
				})
				.encrypt_text_file(thumbs.to_owned())
				.map_err(|x| eyre!("XTEA encryption error: {:?}", x))?
			)?;
		}

		fs::create_dir_all(&self.retail_path)?;
		fs::write(
			self.retail_path.join("thumbs.dat"),
			Xtea::new(if self.version == GlacierGame::FL {
				XteaConfig::KNT
			} else {
				XteaConfig::Woa
			})
			.encrypt_text_file(thumbs.to_owned())
			.map_err(|x| eyre!("XTEA encryption error: {:?}", x))?
		)?;
	}

	#[try_fn]
	fn emit_rpkg(&self, package: (PartitionId, PatchId), contents: &[u8]) -> Result<()> {
		fs::create_dir_all(&self.runtime_path)?;
		fs::write(self.runtime_path.join(package.0.to_filename(package.1)), contents)?;
	}
}

#[derive(Debug, Clone)]
pub struct Filesystem {
	mods_folder: PathBuf,
	mods: Vec<ModID>,
	manifests: PapayaMap<ModID, Arc<Manifest>>,
	partition_names: PapayaSet<EcoString>
}

impl Filesystem {
	#[try_fn]
	pub fn new(mods_folder: impl AsRef<Path>) -> Result<Self> {
		let mods_folder = mods_folder.as_ref().to_owned();

		let mods: Vec<ModID> = fs::read_dir(&mods_folder)?
			.collect::<Result<Vec<_>, _>>()?
			.into_iter()
			.filter(|x| x.file_name() != "DO NOT TOUCH THIS FOLDER")
			.sorted_by_key(|x| x.file_name())
			.map(|x| {
				ModID::try_from(EcoString::from(x.file_name().to_string_lossy()))
					.map_err(|e| eyre!("Invalid mod ID {}: {}", x.file_name().to_string_lossy(), e))
			})
			.collect::<Result<_>>()?;

		Self {
			mods,
			mods_folder,
			partition_names: Default::default(),
			manifests: Default::default()
		}
	}

	pub fn mods_folder(&self) -> &PathBuf {
		&self.mods_folder
	}
}

impl Mods for Filesystem {
	#[try_fn]
	#[wrap_err("Couldn't get mods")]
	fn get_all_mods(&self) -> Result<Vec<ModID>> {
		self.mods.to_owned()
	}

	fn get_mod_root(&self, id: &ModID) -> Option<PathBuf> {
		Some(self.mods_folder.join(id.as_str()))
	}

	#[try_fn]
	#[wrap_err("Couldn't get manifest for {}", id.as_str())]
	fn get_mod_manifest(&self, id: &ModID) -> Result<Arc<Manifest>> {
		let manifests = self.manifests.pin();

		if let Some(cached) = manifests.get(id) {
			cached.clone()
		} else {
			let manifest_path = self.mods_folder.join(id.as_str()).join("manifest.json");

			let manifest = serde_path_to_error::deserialize::<_, Manifest>(&mut Deserializer::from_slice(
				&fs::read(manifest_path)
					.wrap_err_with(|| format!("Couldn't read manifest file for {}", id.as_str()))?
			))
			.wrap_err_with(|| format!("Couldn't parse manifest file for {}", id.as_str()))?;

			if manifest.id != *id {
				bail!(
					"Mismatched mod ID between folder name ({}) and manifest ({})",
					id,
					manifest.id
				);
			}

			let manifest = Arc::new(manifest);
			manifests.insert(id.to_owned(), manifest.clone());
			manifest
		}
	}

	#[try_fn]
	#[wrap_err("Couldn't read file {} in mod {}", path.as_str(), id.as_str())]
	fn read_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<Vec<u8>> {
		let mod_path = self.get_mod_root(id).unwrap();
		fs::read(path.to_logical_path(mod_path))?
	}

	#[try_fn]
	#[wrap_err("Couldn't read content folder {} in mod {}", content_folder.as_str(), id.as_str())]
	#[instrument(skip_all)]
	fn read_mod_content_folder(
		&self,
		id: &ModID,
		content_folder: &RelativePath
	) -> Result<Vec<(NominalPartition, Vec<RelativePathBuf>)>> {
		let mod_path = self.get_mod_root(id).unwrap();

		let mut partition_folders = content_folder
			.to_logical_path(&mod_path)
			.read_dir()
			.wrap_err("Couldn't read content folder entries")?
			.collect::<Result<Vec<_>, _>>()?;

		partition_folders.sort_by_key(|x| x.file_name());

		partition_folders
			.into_par_iter()
			.map(|partition_folder| {
				if partition_folder.file_type()?.is_dir() {
					let partition_folder_name = partition_folder.file_name();
					let partition_folder_name = partition_folder_name.to_str().expect("Non UTF-8 file name");

					if !self.partition_names.pin().contains(partition_folder_name) {
						self.partition_names.pin().insert(partition_folder_name.into());
					}

					Ok((
						NominalPartition(
							self.partition_names
								.pin()
								.get(partition_folder_name)
								.unwrap()
								.to_owned()
						),
						WalkDir::new(partition_folder.path())
							.sort_by_file_name()
							.into_iter()
							.map(|entry| {
								let entry = entry.wrap_err("Error reading partition folder")?;

								if entry.file_type().is_file() && entry.path().extension().is_some() {
									Ok(Some(entry.path().relative_to(&mod_path)?))
								} else {
									Ok(None)
								}
							})
							.flatten_ok()
							.collect::<Result<Vec<_>>>()?
					))
				} else {
					intentional_halt!(
						self.get_mod_manifest(id)?.id,
						"Content folders should contain partition folders, not files"
					);
				}
			})
			.collect::<Result<_>>()?
	}

	#[try_fn]
	#[wrap_err("Couldn't read blob folder {} in mod {}", blob_folder.as_str(), id.as_str())]
	#[instrument(skip_all)]
	fn read_mod_blob_folder(&self, id: &ModID, blob_folder: &RelativePath) -> Result<Vec<(String, RelativePathBuf)>> {
		let mod_path = self.get_mod_root(id).unwrap();
		let blob_folder = blob_folder.to_logical_path(&mod_path);

		WalkDir::new(&blob_folder)
			.sort_by_file_name()
			.into_iter()
			.map(|entry| {
				let entry = entry.wrap_err("Error reading blob folder")?;

				if entry.file_type().is_file() && !entry.file_name().to_string_lossy().ends_with(".ini") {
					Ok(Some((
						entry.path().relative_to(&blob_folder)?.to_string().to_lowercase(),
						entry.path().relative_to(&mod_path)?
					)))
				} else {
					Ok(None)
				}
			})
			.collect::<Result<Vec<_>>>()?
			.into_iter()
			.flatten()
			.collect()
	}
}

impl ModsWritable for Filesystem {
	#[try_fn]
	#[wrap_err("Couldn't write manifest for {}", id.as_str())]
	fn write_mod_manifest(&self, id: &ModID, manifest: Manifest) -> Result<()> {
		if manifest.id != *id {
			bail!("Tried to write manifest with ID {} to mod {}", manifest.id, id);
		}

		let mod_path = self.get_mod_root(id).unwrap();
		fs::write(
			mod_path.join("manifest.json"),
			format_json(&serde_json::to_string(&manifest)?)?
		)?;

		self.manifests.pin().insert(id.to_owned(), manifest.into());
	}

	#[try_fn]
	#[wrap_err("Couldn't write to file {} in mod {}", path.as_str(), id.as_str())]
	fn write_mod_file(&self, id: &ModID, path: &RelativePath, contents: &[u8]) -> Result<()> {
		let mod_path = self.get_mod_root(id).unwrap();
		fs::write(path.to_logical_path(mod_path), contents)?;
	}

	#[try_fn]
	#[wrap_err("Couldn't remove file {} in mod {}", path.as_str(), id.as_str())]
	fn remove_mod_file(&self, id: &ModID, path: &RelativePath) -> Result<()> {
		let mod_path = self.get_mod_root(id).unwrap();
		fs::remove_file(path.to_logical_path(mod_path))?;
	}
}
