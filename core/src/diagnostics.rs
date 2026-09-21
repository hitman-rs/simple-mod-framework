use std::fmt::{Display, Formatter};

use glacier_commons::metadata::{ReferenceType, RuntimeID};
use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};
use simple_mod_framework_types::{ModID, VersionPlatform};
use specta::Type;

use crate::game::ResourceSpecifier;

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DiagnosticSeverity {
	Low,
	Medium,
	High
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DiagnosticTarget {
	None,
	Mod {
		#[serde(rename = "modId")]
		mod_id: ModID
	},
	Operation {
		/// The source of this node; the mod's ID.
		#[serde(rename = "modId")]
		mod_id: ModID,

		/// The file which produced this node, relative to the mod root.
		#[specta(type = String)]
		source: RelativePathBuf,

		/// If this node was produced by a script, a unique identifier for the operation distinguishing it from others from the same script.
		#[serde(rename = "scriptIdentifier")]
		script_identifier: Option<String>
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Diagnostic {
	pub kind: DiagnosticKind,
	pub target: DiagnosticTarget
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DiagnosticKind {
	ResourceAlreadyIdentical {
		resource: ResourceSpecifier
	},
	UnsupportedPlatform {
		platform: VersionPlatform,
		supported: Vec<VersionPlatform>
	},
	ApplyingMigration {
		message: String
	},
	UnrecognizedPatch {
		file_name: String
	},
	UnrecognizedPartitionPatch {
		file_name: String
	},
	QuickEntityPatchDiagnostic {
		diagnostic: String
	},
	QuickEntityPatchNoEffect,
	DestinationsItemNotFoundBefore {
		identifier: String
	},
	DestinationsItemNotFoundAfter {
		identifier: String
	},
	LocalisationOverrideSameValue {
		lang: String,
		key: String
	},
	ScriptDebug {
		message: String
	},
	ReferencedResourceNonexistent {
		resource_id: RuntimeID,
		reference_type: ReferenceType
	},
	ReferencedResourceDeleted {
		resource_id: RuntimeID,
		reference_type: ReferenceType
	},
	PackageDefinitionPlatformPath {
		suffix: String
	}
}

impl DiagnosticKind {
	pub fn severity(&self) -> DiagnosticSeverity {
		match self {
			DiagnosticKind::UnsupportedPlatform { .. } => DiagnosticSeverity::Low,
			DiagnosticKind::ApplyingMigration { .. } => DiagnosticSeverity::Low,

			DiagnosticKind::ResourceAlreadyIdentical { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::UnrecognizedPatch { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::UnrecognizedPartitionPatch { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::QuickEntityPatchDiagnostic { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::QuickEntityPatchNoEffect => DiagnosticSeverity::Medium,
			DiagnosticKind::DestinationsItemNotFoundBefore { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::DestinationsItemNotFoundAfter { .. } => DiagnosticSeverity::Medium,
			DiagnosticKind::LocalisationOverrideSameValue { .. } => DiagnosticSeverity::Medium,

			DiagnosticKind::ReferencedResourceNonexistent { reference_type, .. } => {
				if *reference_type == ReferenceType::Install {
					DiagnosticSeverity::High
				} else {
					DiagnosticSeverity::Medium
				}
			}

			DiagnosticKind::ReferencedResourceDeleted { reference_type, .. } => {
				if *reference_type == ReferenceType::Install {
					DiagnosticSeverity::High
				} else {
					DiagnosticSeverity::Medium
				}
			}

			DiagnosticKind::ScriptDebug { .. } => DiagnosticSeverity::High,
			DiagnosticKind::PackageDefinitionPlatformPath { .. } => DiagnosticSeverity::High
		}
	}
}

impl Display for DiagnosticKind {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			DiagnosticKind::ResourceAlreadyIdentical { resource } => {
				write!(
					f,
					"Resource {:?} is already identical; this operation has no effect.",
					resource
				)
			}

			DiagnosticKind::UnsupportedPlatform { platform, supported } => {
				write!(
					f,
					"This mod may not have been designed for {}. If you experience issues, try playing on {}.",
					platform,
					if supported.len() == 1 {
						supported[0].to_string()
					} else {
						format!(
							"{} or {}",
							supported
								.iter()
								.take(supported.len() - 1)
								.map(|x| x.to_string())
								.collect::<Vec<_>>()
								.join(", "),
							supported.last().unwrap()
						)
					}
				)
			}

			DiagnosticKind::ApplyingMigration { message } => {
				write!(f, "{}", message)
			}

			DiagnosticKind::UnrecognizedPatch { file_name } => {
				write!(
					f,
					"There is an unrecognised patch in your Runtime folder: {}. This could cause issues in-game and \
					 with other mods; remove the RPKG mod and re-add it through SMF with the Add Mod button for the \
					 best experience.",
					file_name
				)
			}

			DiagnosticKind::UnrecognizedPartitionPatch { file_name } => {
				write!(
					f,
					"There is a patch for an unrecognised partition in your Runtime folder: {}. This could cause \
					 issues in-game and with other mods; remove the RPKG mod and re-add it through SMF with the Add \
					 Mod button for the best experience.",
					file_name
				)
			}

			DiagnosticKind::QuickEntityPatchDiagnostic { diagnostic, .. } => {
				write!(f, "In patching entity: {diagnostic}")
			}

			DiagnosticKind::QuickEntityPatchNoEffect => {
				write!(f, "The entity is already identical; this patch has no effect.")
			}

			DiagnosticKind::DestinationsItemNotFoundBefore { identifier } => {
				write!(f, "Couldn't find destinations item {} to place before", identifier)
			}

			DiagnosticKind::DestinationsItemNotFoundAfter { identifier } => {
				write!(f, "Couldn't find destinations item {} to place after", identifier)
			}

			DiagnosticKind::LocalisationOverrideSameValue { lang, key } => {
				write!(f, "Overriding existing localisation for {lang} with same value: {key}")
			}

			DiagnosticKind::ScriptDebug { message, .. } => {
				write!(f, "dbg called with arguments: {message}")
			}

			DiagnosticKind::ReferencedResourceNonexistent {
				resource_id,
				reference_type
			} => {
				write!(
					f,
					"Referenced resource {} does not exist and will be unavailable at runtime. {}",
					resource_id,
					if *reference_type == ReferenceType::Install {
						"This will likely cause game crashes."
					} else {
						"This could cause instability."
					}
				)
			}

			DiagnosticKind::ReferencedResourceDeleted {
				resource_id,
				reference_type
			} => {
				write!(
					f,
					"Referenced resource {} is present only as a deleted resource and will be unavailable at runtime. \
					 {} Use portResources to restore it as-is or replace the reference with a valid one.",
					resource_id,
					if *reference_type == ReferenceType::Install {
						"This will likely cause game crashes."
					} else {
						"This could cause instability."
					}
				)
			}

			DiagnosticKind::PackageDefinitionPlatformPath { suffix } => {
				write!(
					f,
					"The specified package definition path includes a platform suffix .pc_{suffix}, which is almost \
					 certainly wrong; consider changing it to just .{suffix}"
				)
			}
		}
	}
}
