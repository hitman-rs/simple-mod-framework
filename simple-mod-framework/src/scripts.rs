use std::{
	fmt::Debug,
	ops::{Deref, DerefMut},
	str::FromStr,
	sync::Arc
};

use color_eyre::{
	Section, SectionExt,
	eyre::{Result, WrapErr, eyre}
};
use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_commons::{game::GlacierGame, metadata::RuntimeID};
use rune::{
	Context, Diagnostics as RuneDiagnostics, Module, Source, Sources, TypeHash, Unit, Vm,
	runtime::{Function, RuntimeContext}
};
use semver::Version;
use serde::{Deserialize, Serialize};
use simple_mod_framework_core::{
	game::{GameContext, NominalPartition, ResourceSpecifier},
	utils::ResultExt
};
use simple_mod_framework_types::{
	Config, HashMap, ManifestData, ModID, ModOptionID, ModReference, SafeRelativePath, ScriptError, VersionPlatform
};
use tracing::instrument;
use tryvial::try_fn;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
	diagnostics::{Diagnostic, DiagnosticKind},
	graph::{Attribution, Operation, OperationContext, RUNE_OPERATIONS, ScriptOperation},
	world::{Diagnostics, Mods}
};

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, better_rune_derive::Any)]
#[serde(transparent)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT)]
#[rune_functions(Self::r_from, Self::r_from_str, Self::r_to_string, Self::r_get, Self::r_set)]
/// A wrapper around the serde_json::Value type which makes Rune integration easier.
pub struct JsonValue(serde_json::Value);

impl JsonValue {
	#[rune::function(path = Self::from)]
	fn r_from(value: rune::Value) -> Self {
		Self(serde_json::to_value(value).unwrap_or(serde_json::Value::Null))
	}

	#[rune::function(path = Self::from_str)]
	fn r_from_str(value: &str) -> Result<Self, ScriptError> {
		Ok(Self(serde_json::from_str(value)?))
	}

	#[rune::function(instance, path = Self::to_string)]
	fn r_to_string(&self) -> String {
		serde_json::to_string(&self.0).unwrap()
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> rune::Value {
		serde_json::from_value::<rune::Value>(self.0.clone()).unwrap_or(rune::Value::empty())
	}

	#[rune::function(instance, path = Self::set)]
	fn r_set(&mut self, value: rune::Value) {
		self.0 = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
	}
}

impl From<serde_json::Value> for JsonValue {
	fn from(value: serde_json::Value) -> Self {
		Self(value)
	}
}

impl From<JsonValue> for serde_json::Value {
	fn from(value: JsonValue) -> Self {
		value.0
	}
}

impl Deref for JsonValue {
	type Target = serde_json::Value;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for JsonValue {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[try_fn]
pub fn smf_module() -> Result<Module> {
	let mut module = Module::with_crate("simple_mod_framework")?;

	simple_mod_framework_types::rune_install(&mut module)?;
	simple_mod_framework_core::rune_install(&mut module)?;

	module.ty::<Attribution>()?;
	module.ty::<crate::graph::Operation>()?;
	module.ty::<crate::state::ResourceState>()?;
	module.ty::<crate::state::Mutation>()?;
	module.ty::<JsonValue>()?;
	module.ty::<ConditionContext>()?;
	module.ty::<AnalysisContext>()?;
	module.ty::<OperationContext>()?;

	for op in RUNE_OPERATIONS {
		op.install(&mut module)?;
	}

	module
}

#[try_fn]
pub fn install_glacier_bin1_modules(context: &mut rune::Context, game: GlacierGame) -> Result<()> {
	let mut module = Module::with_crate("glacier_bin1")?;

	module
		.function(
			"serialize",
			move |ty: &str, val: JsonValue| -> Result<rune::runtime::Bytes, ScriptError> {
				macro_rules! serialize {
					($ty:ty) => {
						match game {
							GlacierGame::H1 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h1::*;
								glacier_bin1::serialize(&serde_json::from_value::<$ty>(val.into())?)
							}

							GlacierGame::H2 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h2::*;
								glacier_bin1::serialize(&serde_json::from_value::<$ty>(val.into())?)
							}

							GlacierGame::H3 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h3::*;
								glacier_bin1::serialize(&serde_json::from_value::<$ty>(val.into())?)
							}

							_ => return Err(eyre!("{ty} type is not supported in FL").into())
						}
						.map_err(|e| e.into())
					};
				}

				match ty {
					"TEMP" => match game {
						GlacierGame::H1 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h1::STemplateEntity
						>(val.into())?),
						GlacierGame::H2 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h2::STemplateEntityFactory
						>(val.into())?),
						GlacierGame::H3 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h3::STemplateEntityFactory
						>(val.into())?),
						GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::fl::STemplateEntityFactory
						>(val.into())?)
					}
					.map_err(|e| e.into()),

					"ECPB" => match game {
						GlacierGame::H1 => return Err(eyre!("ECPB type is not supported in H1").into()),
						GlacierGame::H2 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h2::SExtendedCppEntityBlueprint
						>(val.into())?),
						GlacierGame::H3 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h3::SExtendedCppEntityBlueprint
						>(val.into())?),
						GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::fl::SExtendedCppEntityBlueprint
						>(val.into())?)
					}
					.map_err(|e| e.into()),

					"ORES-activities" => match game {
						GlacierGame::H1 | GlacierGame::H2 => {
							return Err(eyre!("ORES-activities type is not supported in H1 or H2").into());
						}
						GlacierGame::H3 => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::h3::SActivities
						>(val.into())?),
						GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
							glacier_bin1::game::fl::SActivities
						>(val.into())?)
					}
					.map_err(|e| e.into()),

					"VOXL" if game == GlacierGame::H3 => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::h3::SVoxelSpaceData
					>(val.into())?)
					.map_err(|e| e.into()),

					"CBLU" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SCppEntityBlueprint
					>(val.into())?)
					.map_err(|e| e.into()),
					"CPPT" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SCppEntity
					>(val.into())?)
					.map_err(|e| e.into()),
					"CRMD" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SCrowdMapData
					>(val.into())?)
					.map_err(|e| e.into()),
					"ENUM" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SEnumType
					>(val.into())?)
					.map_err(|e| e.into()),
					"GFXF" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SGFxMovieResource
					>(val.into())?)
					.map_err(|e| e.into()),
					"GIDX" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SResourceIndex
					>(val.into())?)
					.map_err(|e| e.into()),
					"KWOR" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SSerializedKeyword
					>(val.into())?)
					.map_err(|e| e.into()),
					"TBLU" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::STemplateEntityBlueprint
					>(val.into())?)
					.map_err(|e| e.into()),
					"TDAT" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::STerrainResource
					>(val.into())?)
					.map_err(|e| e.into()),
					"TDPK" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::STerrainDataPackage
					>(val.into())?)
					.map_err(|e| e.into()),
					"UICB" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SControlTypeInfo
					>(val.into())?)
					.map_err(|e| e.into()),
					"WSGB" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SAudioStateGroupData
					>(val.into())?)
					.map_err(|e| e.into()),
					"WSWB" if game == GlacierGame::FL => glacier_bin1::serialize(&serde_json::from_value::<
						glacier_bin1::game::fl::SAudioSwitchGroupData
					>(val.into())?)
					.map_err(|e| e.into()),

					"AIBB" => serialize!(SBehaviorTreeInfo),
					"AIRG" => serialize!(SReasoningGrid),
					"ASVA" => serialize!(Vec<SPackedAnimSetEntry>),
					"ATMD" => serialize!(ZAMDTake),
					"BMSK" => serialize!(Vec<u32>),
					"CBLU" => serialize!(SCppEntityBlueprint),
					"CPPT" => serialize!(SCppEntity),
					"CRMD" => serialize!(SCrowdMapData),
					"ENUM" => serialize!(SEnumType),
					"GFXF" => serialize!(SGFxMovieResource),
					"GIDX" => serialize!(SResourceIndex),
					"TBLU" => serialize!(STemplateEntityBlueprint),
					"UICB" => serialize!(SControlTypeInfo),
					"VIDB" => serialize!(SVideoDatabaseData),
					"WSGB" => serialize!(SAudioStateGroupData),
					"WSWB" => serialize!(SAudioSwitchGroupData),
					"ORES-blobs" => serialize!(Vec::<SBlobsConfigResourceEntry>),
					"ORES-contracts" => serialize!(Vec::<SContractConfigResourceEntry>),
					"ORES-unlockables" => serialize!(EcoString),
					"ORES-environment" => serialize!(SEnvironmentConfigResource),

					_ => return Err(eyre!("Unsupported type for serialization: {}", ty).into())
				}
				.and_then(|x| rune::runtime::Bytes::try_from(x).map_err(|e| e.into()))
			}
		)
		.build()?;

	module
		.function(
			"deserialize",
			move |ty: &str, val: &[u8]| -> Result<JsonValue, ScriptError> {
				macro_rules! deserialize {
					($ty:ty) => {
						match game {
							GlacierGame::H1 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h1::*;
								serde_json::to_value(&glacier_bin1::deserialize::<$ty>(&val)?).map(Into::into)
							}

							GlacierGame::H2 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h2::*;
								serde_json::to_value(&glacier_bin1::deserialize::<$ty>(&val)?).map(Into::into)
							}

							GlacierGame::H3 => {
								#[allow(unused_imports)]
								use glacier_bin1::game::h3::*;
								serde_json::to_value(&glacier_bin1::deserialize::<$ty>(&val)?).map(Into::into)
							}

							_ => return Err(eyre!("{ty} type is not supported in FL").into())
						}
						.map_err(|e| e.into())
					};
				}

				match ty {
					"TEMP" => match game {
						GlacierGame::H1 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h1::STemplateEntity
						>(val)?),
						GlacierGame::H2 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h2::STemplateEntityFactory
						>(val)?),
						GlacierGame::H3 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h3::STemplateEntityFactory
						>(val)?),
						GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::fl::STemplateEntityFactory
						>(val)?)
					}
					.map(Into::into)
					.map_err(|e| e.into()),

					"ECPB" => match game {
						GlacierGame::H1 => return Err(eyre!("ECPB type is not supported in H1").into()),
						GlacierGame::H2 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h2::SExtendedCppEntityBlueprint
						>(val)?),
						GlacierGame::H3 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h3::SExtendedCppEntityBlueprint
						>(val)?),
						GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::fl::SExtendedCppEntityBlueprint
						>(val)?)
					}
					.map(Into::into)
					.map_err(|e| e.into()),

					"ORES-activities" => match game {
						GlacierGame::H1 | GlacierGame::H2 => {
							return Err(eyre!("ORES-activities type is not supported in H1 or H2").into());
						}
						GlacierGame::H3 => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::h3::SActivities
						>(val)?),
						GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::fl::SActivities
						>(val)?)
					}
					.map(Into::into)
					.map_err(|e| e.into()),

					"VOXL" if game == GlacierGame::H3 => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::h3::SVoxelSpaceData
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),

					"CBLU" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SCppEntityBlueprint
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"CPPT" if game == GlacierGame::FL => {
						serde_json::to_value(&glacier_bin1::deserialize::<glacier_bin1::game::fl::SCppEntity>(val)?)
							.map(Into::into)
							.map_err(|e| e.into())
					}
					"CRMD" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SCrowdMapData
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"ENUM" if game == GlacierGame::FL => {
						serde_json::to_value(&glacier_bin1::deserialize::<glacier_bin1::game::fl::SEnumType>(val)?)
							.map(Into::into)
							.map_err(|e| e.into())
					}
					"GFXF" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SGFxMovieResource
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"GIDX" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SResourceIndex
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"KWOR" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SSerializedKeyword
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"TBLU" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::STemplateEntityBlueprint
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"TDAT" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::STerrainResource
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"TDPK" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::STerrainDataPackage
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"UICB" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SControlTypeInfo
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"WSGB" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SAudioStateGroupData
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"WSWB" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						glacier_bin1::game::fl::SAudioSwitchGroupData
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"ORES-blobs" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						Vec<glacier_bin1::game::fl::SBlobsConfigResourceEntry>
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"ORES-contracts" if game == GlacierGame::FL => serde_json::to_value(&glacier_bin1::deserialize::<
						Vec<glacier_bin1::game::fl::SContractConfigResourceEntry>
					>(val)?)
					.map(Into::into)
					.map_err(|e| e.into()),
					"ORES-unlockables" if game == GlacierGame::FL => {
						serde_json::to_value(&glacier_bin1::deserialize::<EcoString>(val)?)
							.map(Into::into)
							.map_err(|e| e.into())
					}
					"ORES-environment" if game == GlacierGame::FL => {
						serde_json::to_value(&glacier_bin1::deserialize::<
							glacier_bin1::game::fl::SEnvironmentConfigResource
						>(val)?)
						.map(Into::into)
						.map_err(|e| e.into())
					}

					"AIBB" => deserialize!(SBehaviorTreeInfo),
					"AIRG" => deserialize!(SReasoningGrid),
					"ASVA" => deserialize!(Vec<SPackedAnimSetEntry>),
					"ATMD" => deserialize!(ZAMDTake),
					"BMSK" => deserialize!(Vec<u32>),
					"CBLU" => deserialize!(SCppEntityBlueprint),
					"CPPT" => deserialize!(SCppEntity),
					"CRMD" => deserialize!(SCrowdMapData),
					"ENUM" => deserialize!(SEnumType),
					"GFXF" => deserialize!(SGFxMovieResource),
					"GIDX" => deserialize!(SResourceIndex),
					"TBLU" => deserialize!(STemplateEntityBlueprint),
					"UICB" => deserialize!(SControlTypeInfo),
					"VIDB" => deserialize!(SVideoDatabaseData),
					"WSGB" => deserialize!(SAudioStateGroupData),
					"WSWB" => deserialize!(SAudioSwitchGroupData),
					"ORES-blobs" => deserialize!(Vec::<SBlobsConfigResourceEntry>),
					"ORES-contracts" => deserialize!(Vec::<SContractConfigResourceEntry>),
					"ORES-unlockables" => deserialize!(EcoString),
					"ORES-environment" => deserialize!(SEnvironmentConfigResource),

					_ => Err(eyre!("Unsupported type for deserialization: {}", ty).into())
				}
			}
		)
		.build()?;

	context.install(module)?;
}

#[try_fn]
#[wrap_err("Couldn't create script context")]
pub fn script_context() -> Result<Context> {
	let mut context = Context::with_config(false)?;
	context.install(rune_modules::base64::module(false)?)?;
	context.install(rune_modules::json::module(false)?)?;

	context.install(smf_module()?)?;

	glacier_commons::rune_install(&mut context, false)?;
	glacier_formats::rune_install(&mut context)?;
	quickentity_rs::rune_install(&mut context)?;

	context
}

/// Substitute for the standard Rune `::std::io` module to provide `dbg!`
#[try_fn]
#[wrap_err("Couldn't create I/O module")]
pub fn io_module(world: Arc<impl Diagnostics + Send + Sync + 'static>, attribution: Attribution) -> Result<Module> {
	let mut module = Module::with_crate_item("std", ["io"])?;

	module
		.raw_function(
			"dbg",
			move |stack: &mut dyn rune::runtime::Memory,
			      addr: rune::runtime::InstAddress,
			      args: usize,
			      out: rune::runtime::Output|
			      -> rune::runtime::VmResult<()> {
				for value in rune::vm_try!(stack.slice_at(addr, args)) {
					let _ = world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::ScriptDebug {
							message: format!("{value:?}")
						},
						target: attribution.to_diagnostic_target()
					});
				}
				rune::vm_try!(out.store(stack, ()));
				rune::runtime::VmResult::Ok(())
			}
		)
		.build()?;

	module
}

#[try_fn]
#[wrap_err("Couldn't create FS module")]
pub fn fs_module(world: Arc<impl Mods + Send + Sync + 'static>, mod_id: ModID) -> Result<Module> {
	let mut module = Module::with_crate("fs")?;

	// TODO: Enforce case sensitivity

	module
		.function("read", {
			let world = world.clone();
			let mod_id = mod_id.clone();
			move |path: &str| -> Result<rune::runtime::Bytes, ScriptError> {
				world
					.read_mod_file(&mod_id, &SafeRelativePath::from_str(path)?)
					.map_err(|e| e.into())
					.and_then(|x| rune::runtime::Bytes::try_from(x).map_err(|e| e.into()))
			}
		})
		.build()?;

	module
		.function("read_to_string", {
			let world = world.clone();
			let mod_id = mod_id.clone();
			move |path: &str| -> Result<String, ScriptError> {
				String::from_utf8(world.read_mod_file(&mod_id, &SafeRelativePath::from_str(path)?)?)
					.map_err(|e| e.into())
			}
		})
		.build()?;

	module
}

/// Exposes the apply_patch function with warnings emitted as diagnostics.
#[try_fn]
#[wrap_err("Couldn't create QN additions module")]
pub fn qn_additions_module(
	world: Arc<impl Diagnostics + Send + Sync + 'static>,
	attribution: Attribution
) -> Result<Module> {
	let mut module = Module::with_crate("quickentity_rs")?;

	module
		.function("apply_patch", {
			let world = world.clone();
			let attribution = attribution.clone();
			move |entity: &mut quickentity_rs::entity::Entity,
			      patch: quickentity_rs::patch::Patch|
			      -> Result<bool, ScriptError> {
				quickentity_rs::apply_patch(entity, patch, |diagnostic| {
					let _ = world.emit_diagnostic(Diagnostic {
						kind: DiagnosticKind::QuickEntityPatchDiagnostic {
							diagnostic: diagnostic.to_string()
						},
						target: attribution.to_diagnostic_target()
					});
				})
				.map_err(|e| ScriptError::msg(format!("{:?}", e)))
			}
		})
		.build()?;

	module
}

#[derive(Clone, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
pub struct ConditionContext {
	#[rune(get)]
	pub platform: VersionPlatform,

	#[rune(get)]
	pub config: Config
}

#[try_fn]
#[wrap_err("Condition failed")]
#[instrument(skip_all)]
pub fn eval_condition(
	attribution: &str,
	condition: &str,
	enabled_mod_versions: Arc<HashMap<ModID, Version>>,
	ctx: ConditionContext
) -> Result<bool> {
	let mut context = script_context()?;

	// Condition helpers
	let mut module = Module::with_crate("simple_mod_framework")?;

	module
		.function("mod_enabled", {
			let enabled_mod_versions = enabled_mod_versions.clone();
			move |mod_ref: &str| {
				let Ok(mod_ref) = mod_ref.parse::<ModReference>() else {
					return false;
				};

				enabled_mod_versions
					.get(&mod_ref.id)
					.is_some_and(|version| mod_ref.version.matches(version))
			}
		})
		.build()?;

	module
		.function("mod_option", {
			let ctx = ctx.to_owned();
			move |mod_ref: &str, option: &str| -> rune::Value {
				let Ok(mod_ref) = mod_ref.parse::<ModReference>() else {
					return rune::to_value(None::<rune::Value>).unwrap();
				};

				let Ok(option) = ModOptionID::try_from(EcoString::from(option)) else {
					return rune::to_value(None::<rune::Value>).unwrap();
				};

				if !enabled_mod_versions
					.get(&mod_ref.id)
					.is_some_and(|version| mod_ref.version.matches(version))
				{
					return rune::to_value(None::<rune::Value>).unwrap();
				}

				if let Some(data) = ctx
					.config
					.mod_options
					.get(&mod_ref.id)
					.and_then(|options| options.get(&option))
				{
					rune::to_value(Some(serde_json::from_value::<rune::Value>(data.get_value()).unwrap())).unwrap()
				} else {
					rune::to_value(None::<rune::Value>).unwrap()
				}
			}
		})
		.build()?;

	context.install(module)?;

	let runtime = Arc::new(context.runtime()?);

	let script_hash = xxh3_64(condition.as_bytes());

	let prelude = r"
	use simple_mod_framework::*;
	let game = context.platform;
	let config = context.config;
	";

	let mut sources = Sources::new();
	sources.insert(Source::new(
		attribution,
		format!(
			r"use simple_mod_framework::*;
		pub fn condition(context) {{
			{prelude}
			{condition}
		}}"
		)
	)?)?;
	sources.insert(Source::new(
		"smf_internal",
		format!(
			r#"
			pub fn r{script_hash}_display(x) {{ format!("{{}}", x) }}
			pub fn r{script_hash}_dbg(x) {{ format!("{{:?}}", x) }}
			"#
		)
	)?)?;

	let mut diagnostics = RuneDiagnostics::new();

	let result = rune::prepare(&mut sources)
		.with_context(&context)
		.with_diagnostics(&mut diagnostics)
		.build();

	let mut output = rune::termcolor::Buffer::ansi();
	diagnostics.emit(&mut output, &sources)?;

	let Ok(unit) = result else {
		return Err(eyre!("Failed to compile").section(
			String::from_utf8_lossy(output.as_slice())
				.trim()
				.to_owned()
				.header("Compilation output:")
		))
		.intentional();
	};

	let mut vm = Vm::new(runtime, Arc::new(unit));

	let res = vm
		.call(["condition"], (ctx,))
		.map_err(|e| {
			let mut output = rune::termcolor::Buffer::ansi();

			let mut err = eyre!("Error in script execution");

			if e.emit(&mut output, &sources).is_ok() {
				err = err.section(
					String::from_utf8_lossy(output.as_slice())
						.trim()
						.to_owned()
						.header("Script error:")
				);
			} else {
				err = err.section(format!("{e}"));
			}

			err
		})
		.intentional()?;

	match res.as_bool() {
		Ok(x) => x,
		Err(e) => {
			return Err(eyre!(e))
				.note("condition should return a boolean value")
				.wrap_err("Incorrect return type")
				.intentional();
		}
	}
}

#[derive(better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework, install_with = Self::rune_install)]
#[rune_functions(
	Self::operation,
	Self::custom_operation,
	Self::infer_resource_specifier,
	Self::realise_resource_specifier
)]
pub struct AnalysisContext {
	#[rune(get)]
	pub platform: VersionPlatform,

	pub config: Arc<Config>,

	pub game: Arc<GameContext>,

	pub script_hash: u64,

	pub rune_context: (Arc<RuntimeContext>, Arc<Unit>, Arc<Sources>)
}

impl AnalysisContext {
	#[try_fn]
	fn rune_install(module: &mut Module) -> Result<(), rune::ContextError> {
		module.field_function(&rune::runtime::Protocol::GET, "config", |s: &Self| -> Config {
			(*s.config).to_owned()
		})?;
	}

	#[rune::function(instance, path = Self::operation)]
	pub fn operation(&self, id: &str, operation: rune::Value) -> Result<(String, Operation), ScriptError> {
		for op in RUNE_OPERATIONS {
			if operation.type_hash() == op.type_hash() {
				return Ok((
					id.to_owned(),
					op.downcast(operation)
						.wrap_err_with(|| format!("Error interpreting operation value as {}", op.type_name()))?
				));
			}
		}

		Err(eyre!(
			"Value of type {} is not a graph operation",
			operation.into_type_name().into_result().map_err(|e| eyre!(e))?
		)
		.into())
	}

	#[try_fn]
	#[rune::function(instance, path = Self::custom_operation)]
	pub fn custom_operation(
		&self,
		required: Vec<ResourceSpecifier>,
		affected: Vec<ResourceSpecifier>,
		operation: Function
	) -> Result<ScriptOperation, ScriptError> {
		ScriptOperation {
			required,
			affected,
			operation: operation
				.into_sync()
				.map_err(|e| eyre!(e))
				.wrap_err("Script operations can only capture Rune values")?,
			rune_context: self.rune_context.clone(),
			hash: self.script_hash
		}
	}

	#[try_fn]
	#[rune::function(instance, path = Self::infer_resource_specifier)]
	pub fn infer_resource_specifier(&self, id: &mut RuntimeID) -> Result<Option<ResourceSpecifier>, ScriptError> {
		self.game.infer_resource_specifier(*id)?
	}

	#[try_fn]
	#[rune::function(instance, path = Self::realise_resource_specifier)]
	pub fn realise_resource_specifier(
		&self,
		id: &mut RuntimeID,
		partition: &mut NominalPartition
	) -> Result<Option<ResourceSpecifier>, ScriptError> {
		self.game.realise_resource_specifier(*id, partition.to_owned())?
	}
}

#[try_fn]
#[wrap_err("Script data function failed")]
#[instrument(skip_all)]
pub fn eval_script_data(
	config: Arc<Config>,
	world: Arc<impl Mods + Diagnostics + Send + Sync + 'static>,
	game: Arc<GameContext>,
	attribution: Attribution,
	script: String,
	data: &mut ManifestData
) -> Result<Vec<(String, serde_json::Value)>> {
	let mut context = script_context()?;
	context.install(io_module(world.clone(), attribution.to_owned())?)?;
	context.install(qn_additions_module(world.clone(), attribution.to_owned())?)?;
	context.install(fs_module(world.clone(), attribution.mod_id)?)?;
	install_glacier_bin1_modules(&mut context, game.version)?;

	let runtime = Arc::new(context.runtime()?);

	let script_hash = xxh3_64(script.as_bytes());

	let mut sources = Sources::new();
	sources.insert(Source::new(attribution.source, script)?)?;
	sources.insert(Source::new(
		"smf_internal",
		format!(
			r#"
			pub fn r{script_hash}_display(x) {{ format!("{{}}", x) }}
			pub fn r{script_hash}_dbg(x) {{ format!("{{:?}}", x) }}
			"#
		)
	)?)?;

	let mut diagnostics = RuneDiagnostics::new();

	let result = rune::prepare(&mut sources)
		.with_context(&context)
		.with_diagnostics(&mut diagnostics)
		.build();

	let mut output = rune::termcolor::Buffer::ansi();
	diagnostics.emit(&mut output, &sources)?;

	let Ok(unit) = result else {
		return Err(eyre!("Failed to compile").section(
			String::from_utf8_lossy(output.as_slice())
				.trim()
				.to_owned()
				.header("Compilation output:")
		))
		.intentional();
	};

	let mut vm = Vm::new(runtime, Arc::new(unit));

	let sources = Arc::new(sources);

	let res = vm
		.call(
			["data"],
			(
				&mut AnalysisContext {
					platform: (&*game).into(),
					config: config.clone(),
					game: game.clone(),
					script_hash,
					rune_context: (vm.context().clone(), vm.unit().clone(), sources.clone())
				},
				&mut *data
			)
		)
		.map_err(|e| {
			let mut output = rune::termcolor::Buffer::ansi();

			let mut err = eyre!("Error in script execution");

			if e.emit(&mut output, &sources).is_ok() {
				err = err.section(
					String::from_utf8_lossy(output.as_slice())
						.trim()
						.to_owned()
						.header("Script error:")
				);
			} else {
				err = err.section(format!("{e}"));
			}

			err
		})
		.intentional()?;

	if res.type_hash() == Result::<Vec<(String, rune::Value)>, rune::Value>::HASH {
		return rune::from_value::<Result<Vec<(String, rune::Value)>, rune::Value>>(res)
			.wrap_err("Failed to parse data result")?
			.map_err(|e| {
				eyre!("Script returned error").section(if e.type_hash() == anyhow::Error::HASH {
					format!("{:?}", rune::from_value::<anyhow::Error>(e).unwrap()).header("Script error:")
				} else {
					vm.call([format!("r{script_hash}_display").as_str()], (&e,))
						.or_else(|_| vm.call([format!("r{script_hash}_dbg").as_str()], (&e,)))
						.ok()
						.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
						.unwrap_or_else(|| {
							vm.with(|| e.into_type_name())
								.into_result()
								.map(|x| format!("Can't display error of type {x:?}"))
								.unwrap_or_else(|_| "Can't display error".into())
						})
						.header("Script error:")
				})
			})
			.intentional()?
			.into_iter()
			.map(|(k, v)| Ok((k, serde_json::to_value(v)?)))
			.collect::<Result<_>>()
			.intentional();
	}

	match rune::from_value::<Vec<(String, rune::Value)>>(res) {
		Ok(value) => value
			.into_iter()
			.map(|(k, v)| Ok((k, serde_json::to_value(v)?)))
			.collect::<Result<_>>()
			.intentional()?,

		Err(e) => {
			// Re-run to stringify the resulting value
			let res = vm
				.call(
					["data"],
					(
						&mut AnalysisContext {
							platform: (&*game).into(),
							config,
							game,
							script_hash,
							rune_context: (vm.context().clone(), vm.unit().clone(), sources.clone())
						},
						data
					)
				)
				.intentional()
				.wrap_err("Data function returned incorrect type but second run failed")?;

			return Err(eyre!(e).with_note(|| {
				vm.call([format!("r{script_hash}_display").as_str()], (res.to_owned(),))
					.or_else(|_| vm.call([format!("r{script_hash}_dbg").as_str()], (res,)))
					.ok()
					.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
					.map(|v| format!("data function should return Vec<(String, Operation)> but got value: {v}"))
					.unwrap_or_else(|| "data function should return Vec<(String, Operation)>".into())
			}))
			.wrap_err("Incorrect return type")
			.intentional();
		}
	}
}

#[try_fn]
#[wrap_err("Script operations function failed")]
#[instrument(skip_all)]
pub fn eval_script_operations(
	config: Arc<Config>,
	world: Arc<impl Mods + Diagnostics + Send + Sync + 'static>,
	game: Arc<GameContext>,
	attribution: Attribution,
	script: String,
	mut data: ManifestData
) -> Result<Vec<(String, Operation)>> {
	let mut context = script_context()?;
	context.install(io_module(world.clone(), attribution.to_owned())?)?;
	context.install(qn_additions_module(world.clone(), attribution.to_owned())?)?;
	context.install(fs_module(world.clone(), attribution.mod_id)?)?;
	install_glacier_bin1_modules(&mut context, game.version)?;

	let runtime = Arc::new(context.runtime()?);

	let script_hash = xxh3_64(script.as_bytes());

	let mut sources = Sources::new();
	sources.insert(Source::new(attribution.source, script)?)?;
	sources.insert(Source::new(
		"smf_internal",
		format!(
			r#"
			pub fn r{script_hash}_display(x) {{ format!("{{}}", x) }}
			pub fn r{script_hash}_dbg(x) {{ format!("{{:?}}", x) }}
			"#
		)
	)?)?;

	let mut diagnostics = RuneDiagnostics::new();

	let result = rune::prepare(&mut sources)
		.with_context(&context)
		.with_diagnostics(&mut diagnostics)
		.build();

	let mut output = rune::termcolor::Buffer::ansi();
	diagnostics.emit(&mut output, &sources)?;

	let Ok(unit) = result else {
		return Err(eyre!("Failed to compile").section(
			String::from_utf8_lossy(output.as_slice())
				.trim()
				.to_owned()
				.header("Compilation output:")
		))
		.intentional();
	};

	let mut vm = Vm::new(runtime, Arc::new(unit));

	let sources = Arc::new(sources);

	let res = vm
		.call(
			["operations"],
			(
				&mut AnalysisContext {
					platform: (&*game).into(),
					config: config.clone(),
					game: game.clone(),
					script_hash,
					rune_context: (vm.context().clone(), vm.unit().clone(), sources.clone())
				},
				&mut data
			)
		)
		.map_err(|e| {
			let mut output = rune::termcolor::Buffer::ansi();

			let mut err = eyre!("Error in script execution");

			if e.emit(&mut output, &sources).is_ok() {
				err = err.section(
					String::from_utf8_lossy(output.as_slice())
						.trim()
						.to_owned()
						.header("Script error:")
				);
			} else {
				err = err.section(format!("{e}"));
			}

			err
		})
		.intentional()?;

	if res.type_hash() == Result::<Vec<(String, Operation)>, rune::Value>::HASH {
		return rune::from_value::<Result<Vec<(String, Operation)>, rune::Value>>(res)
			.wrap_err("Failed to parse operations result")?
			.map_err(|e| {
				eyre!("Script returned error").section(if e.type_hash() == anyhow::Error::HASH {
					format!("{:?}", rune::from_value::<anyhow::Error>(e).unwrap()).header("Script error:")
				} else {
					vm.call([format!("r{script_hash}_display").as_str()], (&e,))
						.or_else(|_| vm.call([format!("r{script_hash}_dbg").as_str()], (&e,)))
						.ok()
						.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
						.unwrap_or_else(|| {
							vm.with(|| e.into_type_name())
								.into_result()
								.map(|x| format!("Can't display error of type {x:?}"))
								.unwrap_or_else(|_| "Can't display error".into())
						})
						.header("Script error:")
				})
			})
			.intentional();
	}

	match rune::from_value(res) {
		Ok(value) => value,
		Err(e) => {
			// Re-run to stringify the resulting value
			let res = vm
				.call(
					["operations"],
					(
						&mut AnalysisContext {
							platform: (&*game).into(),
							config,
							game,
							script_hash,
							rune_context: (vm.context().clone(), vm.unit().clone(), sources.clone())
						},
						data
					)
				)
				.intentional()
				.wrap_err("Operations function returned incorrect type but second run failed")?;

			return Err(eyre!(e).with_note(|| {
				vm.call([format!("r{script_hash}_display").as_str()], (res.to_owned(),))
					.or_else(|_| vm.call([format!("r{script_hash}_dbg").as_str()], (res,)))
					.ok()
					.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
					.map(|v| format!("operations function should return Vec<(String, Operation)> but got value: {v}"))
					.unwrap_or_else(|| "operations function should return Vec<(String, Operation)>".into())
			}))
			.wrap_err("Incorrect return type")
			.intentional();
		}
	}
}
