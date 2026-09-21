use std::{
	fmt::{self, Debug, Display},
	ops::{Deref, DerefMut},
	path::Path,
	str::FromStr,
	sync::Arc
};

use ecow::EcoString;
use fn_wrap_err::wrap_err;
use glacier_commons::{game::GlacierGame, metadata::RuntimeID};
use indexmap::IndexMap;
use itertools::Itertools;
use lazy_regex::regex_is_match;
use regex::Regex;
use relative_path::{Component, RelativePathBuf};
use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize, de::Visitor};
use serde_json::Value;
use specta::Type;
use url::Url;
use validated_newtype::validated_newtype;
use vec1::Vec1;

use crate::{
	Game, HashSet, ModOptionValue,
	common::{Platform, ScriptError}
};

macro_rules! impl_rkyv {
	($ty:ty) => {
		impl rkyv::Archive for $ty {
			type Archived = rkyv::string::ArchivedString;
			type Resolver = rkyv::string::StringResolver;

			fn resolve(&self, resolver: Self::Resolver, out: rkyv::Place<Self::Archived>) {
				rkyv::string::ArchivedString::resolve_from_str(self.as_str(), resolver, out);
			}
		}

		impl<S> rkyv::Serialize<S> for $ty
		where
			str: rkyv::SerializeUnsized<S>,
			S: rkyv::rancor::Fallible + ?Sized,
			S::Error: rkyv::rancor::Source
		{
			fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
				rkyv::string::ArchivedString::serialize_from_str(self.as_str(), serializer)
			}
		}

		impl<D> rkyv::Deserialize<$ty, D> for rkyv::string::ArchivedString
		where
			D: rkyv::rancor::Fallible + ?Sized,
			D::Error: rkyv::rancor::Source
		{
			fn deserialize(&self, _: &mut D) -> Result<$ty, D::Error> {
				#[derive(Debug)]
				struct StrError(&'static str);

				impl Display for StrError {
					fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
						f.write_str(self.0)
					}
				}

				impl std::error::Error for StrError {}

				EcoString::from(self.as_str())
					.try_into()
					.map_err(StrError)
					.map_err(rkyv::rancor::Source::new)
			}
		}
	};
}

validated_newtype! {
	#[derive(Serialize, PartialEq, Eq, Clone, Hash, Debug, better_rune_derive::Any)]
	#[rune(item = ::simple_mod_framework)]
	#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
	#[rune_functions(Self::r_from, Self::r_get, Self::r_set)]
	EcoString => pub ModID
	if |x| Regex::new(r"^[a-zA-Z0-9]+\.[A-Z0-9][a-zA-Z0-9]*$").unwrap().is_match(x);
	error r"Mod IDs must follow pattern AuthorName.ModName"
}

impl ModID {
	#[rune::function(path = Self::from)]
	#[wrap_err("Couldn't convert {value} to ModID")]
	fn r_from(value: &str) -> Result<Self, ScriptError> {
		EcoString::from(value).try_into().map_err(ScriptError::msg)
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> String {
		self.0.as_str().into()
	}

	#[rune::function(instance, path = Self::set)]
	#[wrap_err("Couldn't convert {value} to ModID")]
	fn r_set(&mut self, value: &str) -> Result<(), ScriptError> {
		self.0 = Self::try_from(EcoString::from(value)).map_err(ScriptError::msg)?.0;

		Ok(())
	}
}

impl_rkyv!(ModID);

impl Display for ModID {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self)
	}
}

impl Type for ModID {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl JsonSchema for ModID {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"ModID".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"pattern": r"^[a-zA-Z0-9]+\.[A-Z0-9][a-zA-Z0-9]*$"
		})
	}
}

validated_newtype! {
	#[derive(
		Serialize,
		PartialEq,
		Eq,
		Default,
		Hash,
		Clone,
		better_rune_derive::Any
	)]
	#[rune(item = ::simple_mod_framework)]
	#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
	#[rune_functions(Self::r_from, Self::r_get, Self::r_set)]
	EcoString => pub NonEmptyString
	if |x: &str| !x.is_empty() && x.trim().len() == x.len();
	error "String must not be empty or contain extra whitespace"
}

impl NonEmptyString {
	#[rune::function(path = Self::from)]
	#[wrap_err("Couldn't convert {value} to NonEmptyString")]
	fn r_from(value: &str) -> Result<Self, ScriptError> {
		EcoString::from(value).try_into().map_err(ScriptError::msg)
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> String {
		self.0.as_str().into()
	}

	#[rune::function(instance, path = Self::set)]
	#[wrap_err("Couldn't convert {value} to NonEmptyString")]
	fn r_set(&mut self, value: &str) -> Result<(), ScriptError> {
		self.0 = Self::try_from(EcoString::from(value)).map_err(ScriptError::msg)?.0;

		Ok(())
	}
}

impl_rkyv!(NonEmptyString);

impl Display for NonEmptyString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self)
	}
}

impl Debug for NonEmptyString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self)
	}
}

impl Type for NonEmptyString {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl JsonSchema for NonEmptyString {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"NonEmptyString".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"minLength": 1
		})
	}
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
#[serde(transparent)]
pub struct NonEmptyVec<T>(pub Vec1<T>);

impl<T> Deref for NonEmptyVec<T> {
	type Target = Vec1<T>;
	fn deref(&self) -> &Vec1<T> {
		&self.0
	}
}

impl<T> DerefMut for NonEmptyVec<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T: Type> Type for NonEmptyVec<T> {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		<Vec<T> as Type>::definition(types)
	}
}

impl<T> JsonSchema for NonEmptyVec<T>
where
	T: JsonSchema
{
	fn schema_name() -> std::borrow::Cow<'static, str> {
		format!("NonEmptyVec_of_{}", T::schema_name()).into()
	}

	fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "array",
			"items": generator.subschema_for::<T>(),
			"minItems": 1
		})
	}
}

validated_newtype! {
	#[derive(Serialize, PartialEq, Eq, Clone, Debug, Hash, better_rune_derive::Any)]
	#[rune(item = ::simple_mod_framework)]
	#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
	#[rune_functions(Self::r_from, Self::r_get, Self::r_set)]
	RelativePathBuf => pub SafeRelativePath
	if |x: &RelativePathBuf| RelativePathBuf::from_path(Path::new(&x.to_string())).is_ok() && x.is_normalized() && !(
		x.to_string().contains('\\') || x.to_string().contains("..") || x.components().any(|x| matches!(x, Component::ParentDir))
	);
	error r"Paths must not escape the mod folder and should use forward slashes (/) as folder separators"
}

impl SafeRelativePath {
	#[rune::function(path = Self::from)]
	#[wrap_err("Couldn't convert {value} to SafeRelativePath")]
	fn r_from(value: &str) -> Result<Self, ScriptError> {
		value.parse().map_err(ScriptError::from)
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> String {
		self.0.to_string()
	}

	#[rune::function(instance, path = Self::set)]
	#[wrap_err("Couldn't convert {value} to SafeRelativePath")]
	fn r_set(&mut self, value: &str) -> Result<(), ScriptError> {
		*self = value.parse()?;

		Ok(())
	}
}

impl FromStr for SafeRelativePath {
	type Err = color_eyre::eyre::Report;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_from(RelativePathBuf::from_path(Path::new(s))?).map_err(color_eyre::eyre::Report::msg)
	}
}

impl Type for SafeRelativePath {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl JsonSchema for SafeRelativePath {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"SafeRelativePath".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"pattern": r"^((?!(\.\.|\\)).)+$"
		})
	}
}

validated_newtype! {
	#[derive(Serialize, PartialEq, Eq, Hash, Clone, better_rune_derive::Any)]
	#[rune(item = ::simple_mod_framework)]
	#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
	#[rune_functions(Self::r_from, Self::r_get, Self::r_set)]
	EcoString => pub ModOptionID
	if |x: &str| !(x.is_empty() || x.contains(':') || x.contains('{') || x.contains('}') || x.contains(' '));
	error "Mod option IDs must not be empty and must not contain colons, braces or spaces"
}

impl ModOptionID {
	#[rune::function(path = Self::from)]
	#[wrap_err("Couldn't convert {value} to ModOptionID")]
	fn r_from(value: &str) -> Result<Self, ScriptError> {
		EcoString::from(value).try_into().map_err(ScriptError::msg)
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> String {
		self.0.as_str().into()
	}

	#[rune::function(instance, path = Self::set)]
	#[wrap_err("Couldn't convert {value} to ModOptionID")]
	fn r_set(&mut self, value: &str) -> Result<(), ScriptError> {
		self.0 = Self::try_from(EcoString::from(value)).map_err(ScriptError::msg)?.0;

		Ok(())
	}
}

impl_rkyv!(ModOptionID);

impl Display for ModOptionID {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self)
	}
}

impl Debug for ModOptionID {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self)
	}
}

impl Type for ModOptionID {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl JsonSchema for ModOptionID {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"ModOptionID".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"minLength": 1,
			"pattern": r"^((?!(:|\}|\{)).)+$"
		})
	}
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug, JsonSchema)]
#[serde(transparent)]
pub struct SemVer(pub Version);

impl Deref for SemVer {
	type Target = Version;
	fn deref(&self) -> &Version {
		&self.0
	}
}

impl DerefMut for SemVer {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Type for SemVer {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl Display for SemVer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0.to_string())
	}
}

/// A mod manifest.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
	/// The mod's ID. Should follow capitalised reverse URI style (AuthorName.ModName). Don't include special characters; numbers are OK. Words should be separated by CamelCase.
	pub id: ModID,

	/// The name of the mod.
	pub name: UIText,

	/// A description of the mod.
	pub description: UIText,

	/// A list of the mod's authors.
	pub authors: NonEmptyVec<NonEmptyString>,

	/// The mod's version, using semantic versioning (X.Y.Z). As applied to mods:
	/// - updates with only bug fixes/patches increment the third number (patch version)
	/// - updates with any new features increment the second number (minor version)
	/// - updates which change the mod's behaviour in relation to other mods increment the first number (major version)
	///
	/// The major version should almost never be changed if the mod has not massively changed, but any change which affects other mods (for example, in compatibility or incompatibility) MUST increment it.
	/// For example, if the new version makes your mod newly compatible with another mod, the major version should be incremented so that any `incompatibilities` keys from other mods no longer affect yours.
	pub version: SemVer,

	/// The earliest version of the framework that this specific version of the mod has been tested against.
	pub framework_version: SemVer,

	/// A link to the mod's primary host (must be HTTPS).
	/// - A mod hosted on Nexus should link to the Nexus page (e.g. https://nexusmods.com/hitman3/mods/453 - you can find the link before releasing the mod by using the preview function).
	/// - A mod hosted on GitHub should use the GitHub repository link (https://github.com/Notexe/Portable-Chair).
	/// - ModWorkshop mods should use their link (https://modworkshop.net/mod/45444).
	///
	/// Each platform has differing support for SMF features.
	/// - GitHub repositories and ModWorkshop mods support fully automatic updating.
	/// - GitHub repositories additionally support proper changelogs for mods, so users can see all the differences (across all releases) between their version and the latest. ModWorkshop does not support changelogs.
	/// - Nexus Mods only allows for telling the user when an update is available, so the other two platforms are preferable.
	///
	/// In short: use GitHub if you can, ModWorkshop if you really don't want to, and Nexus Mods as a last resort.
	pub url: UrlOrNull,

	/// Relevant links for the mod.
	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub links: Links,

	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub conditions: ManifestConditions,

	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub data: ManifestData,

	/// Settings for the mod that can be customised in the Mod Manager.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub options: Vec<ModOption>,

	/// Preset combinations of mod options that can be selected all at once.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub presets: Vec<OptionPreset>
}

fn is_default<T: Default + PartialEq>(val: &T) -> bool {
	*val == T::default()
}

// TODO: UI implementation of links
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, Type, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Links {
	/// A link to documentation about the mod, separate to its host/homepage (e.g. mod wiki).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub info: Option<Url>,

	/// A link to report issues with the mod (e.g. Nexus comments section, GitHub Issues page).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub issues: Option<Url>,

	/// A link to support the mod's development (e.g. Patreon, Ko-fi, GitHub Sponsors).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub support: Option<Url>
}

/// Some amount of text to be shown in the UI. Can be a single string (assumed to be English), or an object specifying different strings for different languages.
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug, Type, JsonSchema, better_rune_derive::Any)]
#[serde(transparent)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune_functions(
	Self::r_english,
	Self::set_english__meta,
	Self::r_french,
	Self::set_french__meta,
	Self::r_italian,
	Self::set_italian__meta,
	Self::r_german,
	Self::set_german__meta,
	Self::r_spanish,
	Self::set_spanish__meta,
	Self::r_spanish_mexico,
	Self::set_spanish_mexico__meta,
	Self::r_portuguese_brazil,
	Self::set_portuguese_brazil__meta,
	Self::r_turkish,
	Self::set_turkish__meta,
	Self::r_polish,
	Self::set_polish__meta,
	Self::r_russian,
	Self::set_russian__meta,
	Self::r_chinese_simplified,
	Self::set_chinese_simplified__meta,
	Self::r_chinese_traditional,
	Self::set_chinese_traditional__meta,
	Self::r_japanese,
	Self::set_japanese__meta,
	Self::r_korean,
	Self::set_korean__meta
)]
pub struct UIText(UITextInner);

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug, Type, JsonSchema)]
#[serde(untagged)]
enum UITextInner {
	Single(NonEmptyString),
	Localised(Arc<TextLines>)
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Default, Clone, Debug, Type, JsonSchema)]
struct TextLines {
	pub english: Option<NonEmptyString>,
	pub french: Option<NonEmptyString>,
	pub italian: Option<NonEmptyString>,
	pub german: Option<NonEmptyString>,
	pub spanish: Option<NonEmptyString>,
	pub spanish_mexico: Option<NonEmptyString>,
	pub portuguese_brazil: Option<NonEmptyString>,
	pub turkish: Option<NonEmptyString>,
	pub polish: Option<NonEmptyString>,
	pub russian: Option<NonEmptyString>,
	pub chinese_simplified: Option<NonEmptyString>,
	pub chinese_traditional: Option<NonEmptyString>,
	pub japanese: Option<NonEmptyString>,
	pub korean: Option<NonEmptyString>
}

macro_rules! impl_ui_text_fn {
	($lang:ident, $lang_friendly:literal, $locale:literal) => {
		impl UIText {
			#[doc = concat!("Get the ", $lang_friendly, " (locale: ", $locale, ") translation of the text, if specified.")]
			pub fn $lang(&self) -> Option<&NonEmptyString> {
				match &self.0 {
					UITextInner::Localised(lines) => lines.$lang.as_ref(),
					_ => None
				}
			}

			mident::mident! {
				#[doc = concat!("Get the ", $lang_friendly, " (locale: ", $locale, ") translation of the text, if specified.")]
				#[rune::function(instance, path = Self::$lang)]
				fn #concat(r_ $lang)(&self) -> Option<NonEmptyString> {
					match &self.0 {
						UITextInner::Localised(lines) => lines.$lang.to_owned(),
						_ => None
					}
				}

				#[doc = concat!("Set the ", $lang_friendly, " (locale: ", $locale, ") translation of the text.")]
				#[rune::function(instance, keep, path = Self::#concat(set_ $lang))]
				pub fn #concat(set_ $lang)(&mut self, value: Option<NonEmptyString>) {
					match &mut self.0 {
						UITextInner::Localised(lines) => {
							Arc::make_mut(lines).$lang = value;
						}

						UITextInner::Single(english) if let Some(value) = value => {
							self.0 = UITextInner::Localised(TextLines {
								english: Some(english.to_owned()),
								$lang: Some(value),
								..Default::default()
							}.into());
						}

						_ => {}
					}
				}
			}
		}
	};
}

impl_ui_text_fn!(french, "French", "fr");
impl_ui_text_fn!(italian, "Italian", "it");
impl_ui_text_fn!(german, "German", "de");
impl_ui_text_fn!(spanish, "Spanish", "es");
impl_ui_text_fn!(spanish_mexico, "Spanish (Mexico)", "es-MX");
impl_ui_text_fn!(portuguese_brazil, "Portuguese (Brazil)", "pt-BR");
impl_ui_text_fn!(turkish, "Turkish", "tr");
impl_ui_text_fn!(polish, "Polish", "pl");
impl_ui_text_fn!(russian, "Russian", "ru");
impl_ui_text_fn!(chinese_simplified, "Chinese (Simplified)", "zh-Hans");
impl_ui_text_fn!(chinese_traditional, "Chinese (Traditional)", "zh-Hant");
impl_ui_text_fn!(japanese, "Japanese", "ja");
impl_ui_text_fn!(korean, "Korean", "ko");

impl UIText {
	/// Construct an English-only UIText from a string.
	pub fn from_english(text: NonEmptyString) -> Self {
		Self(UITextInner::Single(text))
	}

	/// Get the English (locale: en) translation of the text, if specified.
	pub fn english(&self) -> Option<&NonEmptyString> {
		match &self.0 {
			UITextInner::Single(text) => Some(text),
			UITextInner::Localised(lines) => lines.english.as_ref()
		}
	}

	/// Get the English (locale: en) translation of the text, if specified.
	#[rune::function(instance, path = Self::english)]
	fn r_english(&self) -> Option<NonEmptyString> {
		match &self.0 {
			UITextInner::Single(text) => Some(text.to_owned()),
			UITextInner::Localised(lines) => lines.english.to_owned()
		}
	}

	/// Set the English (locale: en) translation of the text.
	#[rune::function(instance, keep, path = Self::set_english)]
	pub fn set_english(&mut self, value: Option<NonEmptyString>) {
		match &mut self.0 {
			UITextInner::Localised(lines) => {
				Arc::make_mut(lines).english = value;
			}

			UITextInner::Single(english) if let Some(value) = value => {
				*english = value;
			}

			_ => {}
		}
	}

	/// Get the first specified localisation string in game order: English, French, Italian, German, Spanish, Russian, Spanish (Mexico), Portuguese (Brazil), Polish, Chinese (Simplified), Japanese, Chinese (Traditional), Korean, Turkish.
	pub fn first_specified(&self) -> Option<&NonEmptyString> {
		self.english()
			.or(self.french())
			.or(self.italian())
			.or(self.german())
			.or(self.spanish())
			.or(self.russian())
			.or(self.spanish_mexico())
			.or(self.portuguese_brazil())
			.or(self.polish())
			.or(self.chinese_simplified())
			.or(self.japanese())
			.or(self.chinese_traditional())
			.or(self.korean())
			.or(self.turkish())
	}

	/// Get the localisation string for a specific UI locale, falling back to the first specified localisation if not found. Panics if there is no specified localisation at all.
	pub fn loc(&self, locale: &str) -> &NonEmptyString {
		match locale {
			"en" => self
				.english()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"fr" => self
				.french()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"it" => self
				.italian()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"de" => self
				.german()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"es" => self
				.spanish()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"ru" => self
				.russian()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"es-MX" => self
				.spanish_mexico()
				.or(self.spanish()) // Fallback es-MX to es
				.or(self.first_specified())
				.expect("No localisation specified"),
			"pt-BR" => self
				.portuguese_brazil()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"tr" => self
				.turkish()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"pl" => self
				.polish()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"zh-Hans" => self
				.chinese_simplified()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"zh-Hant" => self
				.chinese_traditional()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"ja" => self
				.japanese()
				.or(self.first_specified())
				.expect("No localisation specified"),
			"ko" => self
				.korean()
				.or(self.first_specified())
				.expect("No localisation specified"),
			_ => self.first_specified().expect("No localisation specified")
		}
	}
}

// Can't use Option<Url> directly because it would allow the field to be missing
#[derive(Serialize, Clone, Debug, PartialEq, Type)]
#[serde(transparent)]
pub struct UrlOrNull(pub Option<Url>);

impl Deref for UrlOrNull {
	type Target = Option<Url>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'de> Deserialize<'de> for UrlOrNull {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		struct UrlOrNullVisitor;
		impl<'de> Visitor<'de> for UrlOrNullVisitor {
			type Value = UrlOrNull;

			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("a valid URL or null")
			}

			fn visit_str<E>(self, value: &str) -> Result<UrlOrNull, E>
			where
				E: serde::de::Error
			{
				let url = Url::parse(value).map_err(serde::de::Error::custom)?;
				if url.scheme() != "https" {
					return Err(serde::de::Error::custom("URL must be HTTPS"));
				}
				Ok(UrlOrNull(Some(url)))
			}

			fn visit_string<E>(self, value: String) -> Result<UrlOrNull, E>
			where
				E: serde::de::Error
			{
				self.visit_str(&value)
			}

			fn visit_unit<E>(self) -> Result<UrlOrNull, E>
			where
				E: serde::de::Error
			{
				Ok(UrlOrNull(None))
			}

			fn visit_none<E>(self) -> Result<UrlOrNull, E>
			where
				E: serde::de::Error
			{
				Ok(UrlOrNull(None))
			}
		}

		deserializer.deserialize_any(UrlOrNullVisitor)
	}
}

impl JsonSchema for UrlOrNull {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"UrlOrNull".into()
	}

	fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"oneOf": [
				generator.subschema_for::<url::Url>(),
				{
					"type": "null"
				}
			]
		})
	}
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct OptionPreset {
	/// An ID for the preset. Can be anything, so long as it's not duplicated.
	pub id: ModOptionID,

	/// The name of the preset.
	pub name: UIText,

	/// A description of the preset. Can contain multiple sentences.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<UIText>,

	/// An image representing the preset. Should usually be a gameplay image showing the preset in use.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub image: Option<SafeRelativePath>,

	/// The values of the mod's options for this preset. Options not included will be unchanged.
	pub values: IndexMap<ModOptionID, Value>
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModOption {
	/// An ID for the option. Can be anything, so long as it's not duplicated. This is also used for substitution in content files; `"bla": "#{option:some-id}"` will be replaced with `"bla": true` and `"bla #{option:some-id} bla"` with `"bla true bla"`.
	pub id: ModOptionID,

	#[serde(flatten)]
	pub data: ModOptionData
}

impl ModOption {
	pub fn get_option_by_id<'a>(opts: &'a [ModOption], opt_id: &ModOptionID) -> Option<&'a ModOption> {
		opts.iter().find_map(|opt| {
			if opt.id == *opt_id {
				Some(opt)
			} else {
				match &opt.data {
					ModOptionData::OptionGroup { options, .. } => Self::get_option_by_id(options, opt_id),
					_ => None
				}
			}
		})
	}

	pub fn all_option_ids(opts: &[ModOption]) -> Vec<&ModOptionID> {
		opts.iter()
			.flat_map(|opt| {
				[&opt.id].into_iter().chain(match &opt.data {
					ModOptionData::OptionGroup { options, .. } => Self::all_option_ids(options),
					_ => Vec::new()
				})
			})
			.collect()
	}
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ModOptionData {
	/// A boolean option. Can be enabled or disabled by a user.
	Boolean {
		/// The name of the option.
		name: UIText,

		/// A description of the option. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met). Preselected options should deliver the "advertised experience". Mods with many standalone options should have the user select their own settings, or provide a conservative default.
		#[serde(rename = "defaultValue")]
		default_value: bool,

		/// An image representing the option. Should usually be a gameplay image showing the option in use.
		#[serde(skip_serializing_if = "Option::is_none")]
		image: Option<SafeRelativePath>,

		/// Conditions which must be met for this option to be configurable.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		conditions: ManifestConditions,

		/// Data to conditionally deploy when this option's value is true.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	},

	/// A selection group, where the user selects only one option to enable from a list of options.
	Selection {
		/// The name of the selection group.
		name: UIText,

		/// A description of the selection group. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// The default-selected option's ID. Will also be used if the selection group itself is disabled (e.g. if it is part of a group whose `displayCondition` is not met).
		#[serde(rename = "defaultValue")]
		default_value: ModOptionID,

		/// Conditions which must be met for this option to be configurable.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		conditions: ManifestConditions,

		/// Data to conditionally deploy when this option is configurable (when all conditions are met).
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData,

		/// The available options to choose from.
		options: NonEmptyVec<SelectionOption>
	},

	/// A conditional option. This is a special case; conditional options are not shown to the user, and are instead enabled if and only if the given condition is met.
	Conditional {
		/// A condition written in Rune which will enable the option when met.
		/// Should be formatted as an expression.
		/// Some helper functions are given in the global scope.
		/// The framework config and game (version and platform) are available in the global scope as `config` and `game`.
		/// For example, `mod_enabled("Author.SomeMod@1.0.0") && game.platform == Platform::Steam`.
		condition: NonEmptyString,

		/// Data to conditionally deploy when this option's condition evaluates to true.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	},

	/// A number option. The user can enter a number (optionally with validation).
	Number {
		/// The name of the option.
		name: UIText,

		/// A description of the option. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
		#[serde(rename = "defaultValue")]
		default_value: f64,

		/// Validation settings for this option.
		#[serde(default)]
		validation: NumberOptionValidation,

		/// An image representing the option. Should usually be a gameplay image showing what effect the option might have.
		#[serde(skip_serializing_if = "Option::is_none")]
		image: Option<SafeRelativePath>,

		/// Conditions which must be met for this option to be configurable.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		conditions: ManifestConditions,

		/// Data to conditionally deploy when this option is configurable (when all conditions are met).
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	},

	/// A colour option. The user can select a colour using a visual picker.
	Color {
		/// The name of the option.
		name: UIText,

		/// A description of the option. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
		#[serde(rename = "defaultValue")]
		#[specta(type = String)]
		#[schemars(with = "String")]
		default_value: EcoString,

		/// Whether the colour should include a transparency/alpha value. If disabled (default), the value of this option is of the form "#rrggbb". If enabled, it is "#rrggbbaa".
		#[serde(default)]
		alpha: bool,

		/// An image representing the option. Should usually be a gameplay image showing what effect the option might have.
		#[serde(skip_serializing_if = "Option::is_none")]
		image: Option<SafeRelativePath>,

		/// Conditions which must be met for this option to be configurable.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		conditions: ManifestConditions,

		/// Data to conditionally deploy when this option is configurable (when all conditions are met).
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	},

	/// A number option. The user can enter a freeform string (optionally with validation).
	String {
		/// The name of the option.
		name: UIText,

		/// A description of the option. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
		#[serde(rename = "defaultValue")]
		#[specta(type = String)]
		#[schemars(with = "String")]
		default_value: EcoString,

		/// Validation settings for this option.
		#[serde(default)]
		validation: StringOptionValidation,

		/// An image representing the option. Should usually be a gameplay image showing what effect the option might have.
		#[serde(skip_serializing_if = "Option::is_none")]
		image: Option<SafeRelativePath>,

		/// Conditions which must be met for this option to be configurable.
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		conditions: ManifestConditions,

		/// Data to conditionally deploy when this option is configurable (when all conditions are met).
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	},

	/// A group of related options, for visual formatting.
	OptionGroup {
		/// This entire option group will only be displayed (and only be configurable) if the given condition is met. Should be formatted as an expression; see the documentation for examples.
		#[serde(skip_serializing_if = "Option::is_none")]
		#[serde(rename = "displayCondition")]
		display_condition: Option<NonEmptyString>,

		/// The name of the group.
		name: UIText,

		/// A description of the group. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		description: Option<UIText>,

		/// A description of the group, to be shown when the display condition is NOT met. Can contain multiple sentences.
		#[serde(skip_serializing_if = "Option::is_none")]
		#[serde(rename = "hiddenDescription")]
		hidden_description: Option<UIText>,

		/// The options contained within this group.
		options: NonEmptyVec<ModOption>,

		/// Preset combinations of mod options that can be selected all at once.
		#[serde(default)]
		#[serde(skip_serializing_if = "Vec::is_empty")]
		presets: Vec<OptionPreset>,

		/// Data to conditionally deploy when this option group is displayed (when its condition is met).
		#[serde(default)]
		#[serde(skip_serializing_if = "is_default")]
		data: ManifestData
	}
}

impl ModOptionData {
	pub fn validate(&self, value: &ModOptionValue) -> ValidationResult {
		match self {
			ModOptionData::Boolean { .. } => {
				if let ModOptionValue::Boolean { .. } = value {
					ValidationResult::Pass
				} else {
					ValidationResult::Fail("Expected a boolean value".into())
				}
			}

			ModOptionData::Selection { options, .. } => {
				if let ModOptionValue::Selection { value } = value {
					if options.iter().any(|option| option.id == *value) {
						ValidationResult::Pass
					} else {
						ValidationResult::Fail(format!("No such option: {value}"))
					}
				} else {
					ValidationResult::Fail("Expected a selection value".into())
				}
			}

			ModOptionData::Number { validation, .. } => {
				if let ModOptionValue::Number { value } = value {
					validation.validate(*value)
				} else {
					ValidationResult::Fail("Expected a number value".into())
				}
			}

			ModOptionData::Color { alpha, .. } => {
				if let ModOptionValue::Color { value } = value {
					if *alpha {
						if !regex_is_match!(r"^#[0-9a-f]{8}$", &value) {
							ValidationResult::Fail("Value must be a valid hex colour code with alpha".into())
						} else {
							ValidationResult::Pass
						}
					} else {
						if !regex_is_match!(r"^#[0-9a-f]{6}$", &value) {
							ValidationResult::Fail("Value must be a valid hex colour code without alpha".into())
						} else {
							ValidationResult::Pass
						}
					}
				} else {
					ValidationResult::Fail("Expected a color value".into())
				}
			}

			ModOptionData::String { validation, .. } => {
				if let ModOptionValue::String { value } = value {
					validation.validate(value)
				} else {
					ValidationResult::Fail("Expected a string value".into())
				}
			}

			ModOptionData::Conditional { .. } | ModOptionData::OptionGroup { .. } => ValidationResult::Pass
		}
	}
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionOption {
	/// An ID for the option. Can be anything, so long as it's not duplicated.
	pub id: ModOptionID,

	/// The option's name.
	pub name: UIText,

	/// A description of the option. Can contain multiple sentences.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<UIText>,

	/// An image representing the option. Should usually be a gameplay image showing the option in use.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub image: Option<SafeRelativePath>,

	/// Conditions which must be met for this option to be selectable.
	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub conditions: ManifestConditions,

	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub data: ManifestData
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, Type, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NumberOptionValidation {
	/// The minimum value the user is allowed to enter.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub minimum: Option<f64>,

	/// The minimum value the user is allowed to enter.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub maximum: Option<f64>,

	/// Whether the user must enter an integer (a whole number).
	#[serde(default)]
	#[serde(skip_serializing_if = "is_default")]
	pub integer: bool
}

impl NumberOptionValidation {
	pub fn validate(&self, value: f64) -> ValidationResult {
		if let Some(minimum) = self.minimum.as_ref()
			&& value < *minimum
		{
			return ValidationResult::Fail(format!("Value cannot be less than {minimum}"));
		}

		if let Some(maximum) = self.maximum.as_ref()
			&& value > *maximum
		{
			return ValidationResult::Fail(format!("Value cannot be greater than {maximum}"));
		}

		if self.integer && value.trunc() != value {
			return ValidationResult::Fail("Value must be an integer (a whole number)".into());
		}

		ValidationResult::Pass
	}
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, Type, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StringOptionValidation {
	/// A regular expression that the user's input must match. Should be a valid Rust regex. For example, `^[a-zA-Z0-9]+$` to require a non-empty string with only alphanumeric characters.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pattern: Option<String>,

	/// The minimum length of the user's input.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub min_length: Option<usize>,

	/// The maximum length of the user's input.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_length: Option<usize>
}

impl StringOptionValidation {
	pub fn validate(&self, value: &str) -> ValidationResult {
		if let Some(pattern) = self.pattern.as_ref() {
			let Ok(pattern) = Regex::new(pattern) else {
				return ValidationResult::Fail(format!("Invalid regular expression: /{pattern}/"));
			};

			if !pattern.is_match(value) {
				return ValidationResult::Fail(format!("Value must match pattern /{pattern}/"));
			}
		}

		if let Some(min_length) = self.min_length.as_ref()
			&& value.chars().count() < *min_length
		{
			return ValidationResult::Fail(format!("Value cannot be shorter than {min_length} characters"));
		}

		if let Some(max_length) = self.max_length.as_ref()
			&& value.chars().count() > *max_length
		{
			return ValidationResult::Fail(format!("Value cannot be longer than {max_length} characters"));
		}

		ValidationResult::Pass
	}
}

#[derive(Serialize, Deserialize, Type)]
#[serde(tag = "result", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ValidationResult {
	Pass,
	Fail(String)
}

impl ValidationResult {
	pub fn wrap_fail(self, msg: impl Display) -> Self {
		match self {
			ValidationResult::Pass => self,
			ValidationResult::Fail(existing) => Self::Fail(format!("{}:\n{}", msg, existing))
		}
	}
}

#[derive(Serialize, Deserialize, Default, JsonSchema, Clone, Debug, Type, PartialEq, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, CLONE)]
#[rune_functions(Self::r_new)]
pub struct ManifestConditions {
	/// Games that this mod supports. All other games will be considered unsupported by this mod.
	///
	/// You can specify entire game versions (e.g. h1) or specific version/platform combinations (e.g. h3-epic).
	///
	/// Use this when a mod uses features that only some games support, such as Ghost Mode and H2 (plus H3 Steam).
	#[serde(default)]
	#[serde(skip_serializing_if = "HashSet::is_empty")]
	#[serde(serialize_with = "serialise_supported_games")]
	#[serde(deserialize_with = "deserialise_supported_games")]
	#[specta(type = std::collections::HashSet<String>)]
	#[schemars(with = "std::collections::HashSet<String>")]
	pub supported_games: HashSet<VersionPlatform>,

	/// Mods that this mod depends on to function. Clients without these mods enabled will be prevented from using this mod.
	/// Should be specified as `modID@version`, where version can be a simple version (e.g. `1.0.0`, meaning 1.x.x) or a range specifier in standard npm-style syntax (e.g. `^2.0.0` or `1.2.0-1.3.1`).
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub required_mods: Vec<ModReference>,

	/// Conditions which this mod depends on to function. When any condition is not met, the user will be prevented from using this mod.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub required_conditions: Vec<ExplainedCondition>,

	/// Mods that this mod will not function with. Clients with these mods enabled will be prevented from using this mod.
	/// Should be specified as `modID@version`, where version can be a simple version (e.g. `1.0.0`, meaning 1.x.x) or a range specifier in standard npm-style syntax (e.g. `^2.0.0` or `1.2.0-1.3.1`).
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub incompatible_mods: Vec<ModReference>,

	/// Conditions which this mod is incompatible with. When any condition is met, the user will be prevented from using this mod.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub incompatible_conditions: Vec<ExplainedCondition>
}

impl ManifestConditions {
	#[rune::function(path = Self::new)]
	pub fn r_new() -> Self {
		Self::default()
	}
}

#[derive(Serialize, Deserialize, Default, JsonSchema, Clone, Debug, Type, PartialEq, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, CLONE)]
#[rune_functions(Self::r_new, Self::extend__meta)]
pub struct ManifestData {
	/// Folders with content files that will be crawled and automatically deployed.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub content_folders: Vec<SafeRelativePath>,

	/// Folders with blobs that will be crawled and automatically deployed.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub blob_folders: Vec<SafeRelativePath>,

	/// Localisation keys (and their text values) to make globally available.
	#[serde(default)]
	#[serde(skip_serializing_if = "IndexMap::is_empty")]
	pub localisation: IndexMap<NonEmptyString, Localisation>,

	/// LINE files to create from localisation IDs.
	#[serde(default)]
	#[serde(skip_serializing_if = "IndexMap::is_empty")]
	pub localised_lines: IndexMap<RuntimeID, NonEmptyString>,

	/// Paths to add to packagedefinition. Custom partitions are not supported.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub package_definition: Vec<PackageDefinitionEntity>,

	/// Resources that will be made available to a given partition (or super/chunk0 if unspecified).
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub port_resources: Vec<PortedResource>,

	/// Mods that this mod should deploy before. Used in automatic sorting by the Mod Manager.
	/// Should be specified as `modID@version`, where version can be a simple version (e.g. `1.0.0`, meaning 1.x.x) or a range specifier in standard npm-style syntax (e.g. `^2.0.0` or `1.2.0-1.3.1`).
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub deploy_before: Vec<ModReference>,

	/// Mods that this mod should deploy after. Used in automatic sorting by the Mod Manager.
	/// Should be specified as `modID@version`, where version can be a simple version (e.g. `1.0.0`, meaning 1.x.x) or a range specifier in standard npm-style syntax (e.g. `^2.0.0` or `1.2.0-1.3.1`).
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	#[rune(get, set)]
	pub deploy_after: Vec<ModReference>,

	/// Paths to plugins that Peacock should load when this mod is deployed.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub peacock_plugins: Vec<SafeRelativePath>,

	/// Game versions, and the paths to mod DLLs that the appropriate SDK (ZHMModSDK/ZKntSDK) should load when this mod is deployed.
	#[serde(default)]
	#[serde(skip_serializing_if = "IndexMap::is_empty")]
	pub sdk_mods: IndexMap<GlacierGame, Vec<SafeRelativePath>>,

	/// Paths to Rune files that can alter deployment of the mod. The scripting API is currently unstable and may change between framework versions.
	#[serde(default)]
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub scripts: Vec<SafeRelativePath>
}

impl ManifestData {
	#[rune::function(path = Self::new)]
	pub fn r_new() -> Self {
		Self::default()
	}
}

impl ManifestData {
	/// Merge another `ManifestData` into this one, combining their fields.
	#[rune::function(instance, keep, path = Self::extend)]
	pub fn extend(&mut self, other: Self) {
		self.content_folders.extend(
			other
				.content_folders
				.into_iter()
				.filter(|x| !self.content_folders.contains(x))
				.collect_vec()
		);

		self.blob_folders.extend(
			other
				.blob_folders
				.into_iter()
				.filter(|x| !self.blob_folders.contains(x))
				.collect_vec()
		);

		self.package_definition.extend(
			other
				.package_definition
				.into_iter()
				.filter(|x| !self.package_definition.contains(x))
				.collect_vec()
		);

		self.port_resources.extend(
			other
				.port_resources
				.into_iter()
				.filter(|x| !self.port_resources.contains(x))
				.collect_vec()
		);

		self.deploy_before.extend(
			other
				.deploy_before
				.into_iter()
				.filter(|x| !self.deploy_before.contains(x))
				.collect_vec()
		);

		self.deploy_after.extend(
			other
				.deploy_after
				.into_iter()
				.filter(|x| !self.deploy_after.contains(x))
				.collect_vec()
		);

		self.peacock_plugins.extend(
			other
				.peacock_plugins
				.into_iter()
				.filter(|x| !self.peacock_plugins.contains(x))
				.collect_vec()
		);

		for (key, vals) in other.sdk_mods {
			self.sdk_mods.entry(key).or_default().extend(vals);
		}

		self.scripts.extend(
			other
				.scripts
				.into_iter()
				.filter(|x| !self.scripts.contains(x))
				.collect_vec()
		);

		self.localised_lines.extend(other.localised_lines);

		for (key, vals) in other.localisation {
			self.localisation.entry(key).or_default().extend(vals);
		}
	}
}

fn serialise_supported_games<S: serde::Serializer>(
	platforms: &HashSet<VersionPlatform>,
	serializer: S
) -> Result<S::Ok, S::Error> {
	let mut result = vec![];

	let h1 = platforms
		.iter()
		.filter_map(|x| (x.version == GlacierGame::H1).then_some(x.platform))
		.collect_vec();

	let h2 = platforms
		.iter()
		.filter_map(|x| (x.version == GlacierGame::H2).then_some(x.platform))
		.collect_vec();

	let h3 = platforms
		.iter()
		.filter_map(|x| (x.version == GlacierGame::H3).then_some(x.platform))
		.collect_vec();

	let fl = platforms
		.iter()
		.filter_map(|x| (x.version == GlacierGame::FL).then_some(x.platform))
		.collect_vec();

	if [Platform::Epic, Platform::Steam, Platform::Microsoft, Platform::GOG]
		.iter()
		.all(|x| h1.contains(x))
	{
		result.push("h1".to_owned());
	} else {
		result.extend(h1.into_iter().map(|x| {
			format!(
				"{:?}",
				VersionPlatform {
					version: GlacierGame::H1,
					platform: x
				}
			)
		}));
	}

	if [Platform::Epic, Platform::Steam].iter().all(|x| h2.contains(x)) {
		result.push("h2".to_owned());
	} else {
		result.extend(h2.into_iter().map(|x| {
			format!(
				"{:?}",
				VersionPlatform {
					version: GlacierGame::H2,
					platform: x
				}
			)
		}));
	}

	if [Platform::Epic, Platform::Steam, Platform::Microsoft]
		.iter()
		.all(|x| h3.contains(x))
	{
		result.push("h3".to_owned());
	} else {
		result.extend(h3.into_iter().map(|x| {
			format!(
				"{:?}",
				VersionPlatform {
					version: GlacierGame::H3,
					platform: x
				}
			)
		}));
	}

	if [Platform::Epic, Platform::Steam].iter().all(|x| fl.contains(x)) {
		result.push("fl".to_owned());
	} else {
		result.extend(fl.into_iter().map(|x| {
			format!(
				"{:?}",
				VersionPlatform {
					version: GlacierGame::FL,
					platform: x
				}
			)
		}));
	}

	result.serialize(serializer)
}

fn deserialise_supported_games<'de, D: serde::Deserializer<'de>>(
	deserializer: D
) -> Result<HashSet<VersionPlatform>, D::Error>
where
	D::Error: serde::de::Error
{
	let platforms = Vec::<EcoString>::deserialize(deserializer)?;
	let mut result = HashSet::with_capacity_and_hasher(platforms.len(), Default::default());

	for platform in platforms {
		match platform.as_ref() {
			"h1" => {
				for p in [Platform::Epic, Platform::Steam, Platform::Microsoft, Platform::GOG]
					.into_iter()
					.map(|p| VersionPlatform {
						version: GlacierGame::H1,
						platform: p
					}) {
					result
						.insert(p)
						.then_some(())
						.ok_or_else(|| <D::Error as serde::de::Error>::custom("platforms should be unique"))?;
				}
			}

			"h2" => {
				for p in [Platform::Epic, Platform::Steam].into_iter().map(|p| VersionPlatform {
					version: GlacierGame::H2,
					platform: p
				}) {
					result
						.insert(p)
						.then_some(())
						.ok_or_else(|| <D::Error as serde::de::Error>::custom("platforms should be unique"))?;
				}
			}

			"h3" => {
				for p in [Platform::Epic, Platform::Steam, Platform::Microsoft]
					.into_iter()
					.map(|p| VersionPlatform {
						version: GlacierGame::H3,
						platform: p
					}) {
					result
						.insert(p)
						.then_some(())
						.ok_or_else(|| <D::Error as serde::de::Error>::custom("platforms should be unique"))?;
				}
			}

			"fl" => {
				for p in [Platform::Epic, Platform::Steam].into_iter().map(|p| VersionPlatform {
					version: GlacierGame::FL,
					platform: p
				}) {
					result
						.insert(p)
						.then_some(())
						.ok_or_else(|| <D::Error as serde::de::Error>::custom("platforms should be unique"))?;
				}
			}

			platform => {
				result.insert(VersionPlatform::try_from(platform).map_err(<D::Error as serde::de::Error>::custom)?);
			}
		}
	}

	Ok(result)
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, Type, PartialEq, Eq, Hash, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune(constructor)]
pub struct ExplainedCondition {
	/// A condition written in Rune. Should be formatted as an expression.
	/// Some helper functions are given in the global scope.
	/// The framework config and game (version and platform) are available in the global scope as `config` and `game`.
	/// For example, `mod_option("Author.SomeMod@1.0.0", "an-option") == Some("a-value")`.
	#[rune(get, set)]
	pub condition: NonEmptyString,

	/// A short explanation of the condition, to be shown to the user when the condition is not met.
	/// For example, "Incompatible with Lighting Ultimate's Vanilla+ Sapienza" or "Requires either Mod A or Mod B to be enabled".
	/// Should not end with punctuation.
	#[rune(get, set)]
	pub explanation: UIText
}

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Default, Clone, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune_functions(Self::r_from, Self::r_get, Self::r_set)]
#[serde(transparent)]
pub struct VersionRange(pub VersionReq);

impl VersionRange {
	#[rune::function(path = Self::from)]
	#[wrap_err("Couldn't convert {value} to VersionRange")]
	fn r_from(value: &str) -> Result<Self, ScriptError> {
		value.parse().map(Self).map_err(Into::into)
	}

	#[rune::function(instance, path = Self::get)]
	fn r_get(&self) -> String {
		self.0.to_string()
	}

	#[rune::function(instance, path = Self::set)]
	#[wrap_err("Couldn't convert {value} to VersionRange")]
	fn r_set(&mut self, value: &str) -> Result<(), ScriptError> {
		self.0 = value.parse()?;

		Ok(())
	}
}

impl Deref for VersionRange {
	type Target = VersionReq;
	fn deref(&self) -> &VersionReq {
		&self.0
	}
}

impl DerefMut for VersionRange {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Type for VersionRange {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl Display for VersionRange {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0.to_string())
	}
}

#[serde_with::apply(_ => #[rune(get, set)])]
#[derive(Debug, Clone, Type, Hash, PartialEq, Eq, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune(constructor)]
pub struct ModReference {
	pub id: ModID,
	pub version: VersionRange
}

impl JsonSchema for ModReference {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"ModReference".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"pattern": r"^[a-zA-Z0-9]+\.[A-Z0-9][a-zA-Z0-9]*@.+$"
		})
	}
}

impl Display for ModReference {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}@{}", self.id, self.version)
	}
}

impl Serialize for ModReference {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer
	{
		serializer.serialize_str(&self.to_string())
	}
}

impl<'de> Deserialize<'de> for ModReference {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		deserializer.deserialize_str(ModReqVisitor)
	}
}

impl FromStr for ModReference {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let (mod_id, version) = s.split_once('@').ok_or("must have exactly one @")?;

		Ok(ModReference {
			id: ModID::try_from(EcoString::from(mod_id))?,
			version: VersionRange(VersionReq::parse(version).map_err(|x| format!("invalid version range: {}", x))?)
		})
	}
}

struct ModReqVisitor;

impl<'de> Visitor<'de> for ModReqVisitor {
	type Value = ModReference;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		formatter.write_str("a mod ID followed by a version or range in the format mod@version")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error
	{
		v.parse().map_err(E::custom)
	}

	fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
	where
		E: serde::de::Error
	{
		v.parse().map_err(E::custom)
	}
}

#[serde_with::apply(_ => #[rune(get, set)])]
#[derive(PartialEq, Eq, Hash, Clone, Copy, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune(constructor)]
pub struct VersionPlatform {
	pub version: GlacierGame,
	pub platform: Platform
}

impl From<&Game> for VersionPlatform {
	fn from(game: &Game) -> Self {
		VersionPlatform {
			version: game.version,
			platform: game.platform
		}
	}
}

impl Display for VersionPlatform {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{} ({})", self.version, self.platform)
	}
}

impl Debug for VersionPlatform {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}-{}",
			match self.version {
				GlacierGame::H1 => "h1",
				GlacierGame::H2 => "h2",
				GlacierGame::H3 => "h3",
				GlacierGame::FL => "fl"
			},
			match self.platform {
				Platform::Epic => "epic",
				Platform::Steam => "steam",
				Platform::Microsoft => "microsoft",
				Platform::GOG => "gog"
			}
		)
	}
}

impl Type for VersionPlatform {
	fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
		String::definition(types)
	}
}

impl JsonSchema for VersionPlatform {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"VersionPlatform".into()
	}

	fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
		schemars::json_schema!({
			"type": "string",
			"pattern": r"^(h(1|2|3)|fl)(-(epic|steam|microsoft|gog))?$"
		})
	}
}

impl TryFrom<&str> for VersionPlatform {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		let (version, platform) = value.split_once('-').ok_or("invalid (version)-(platform) string")?;

		let version = match version {
			"h1" => GlacierGame::H1,
			"h2" => GlacierGame::H2,
			"h3" => GlacierGame::H3,
			"fl" => GlacierGame::FL,
			_ => return Err("invalid game version".into())
		};

		let platform = match platform {
			"epic" => Platform::Epic,
			"steam" => Platform::Steam,
			"microsoft" => Platform::Microsoft,
			"gog" => Platform::GOG,
			_ => return Err("invalid platform".into())
		};

		if matches!(
			(version, platform),
			(GlacierGame::H2, Platform::Microsoft)
				| (GlacierGame::H2, Platform::GOG)
				| (GlacierGame::H3, Platform::GOG)
				| (GlacierGame::FL, Platform::Microsoft)
				| (GlacierGame::FL, Platform::GOG)
		) {
			return Err(format!("{version} is not available on {platform:?}"));
		}

		Ok(VersionPlatform { version, platform })
	}
}

impl Serialize for VersionPlatform {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer
	{
		serializer.serialize_str(&format!("{:?}", self))
	}
}

impl<'de> Deserialize<'de> for VersionPlatform {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		deserializer.deserialize_str(VersionPlatformVisitor)
	}
}

struct VersionPlatformVisitor;

impl<'de> Visitor<'de> for VersionPlatformVisitor {
	type Value = VersionPlatform;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		formatter.write_str("a game version and platform in the format (version)-(platform), e.g. h3-steam")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error
	{
		VersionPlatform::try_from(v).map_err(E::custom)
	}
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, Type, Hash, PartialEq, Eq, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[serde(untagged)]
pub enum PortedResource {
	#[rune(constructor)]
	Simple(#[rune(get, set)] RuntimeID),

	#[rune(constructor)]
	WithOptions {
		#[rune(get, set)]
		resource: RuntimeID,

		#[serde(rename = "forPartition")]
		#[rune(get, set)]
		for_partition: NonEmptyString
	}
}

#[serde_with::apply(_ => #[rune(get, set)])]
#[derive(
	Serialize,
	Deserialize,
	JsonSchema,
	Clone,
	Debug,
	Type,
	Hash,
	PartialEq,
	Eq,
	better_rune_derive::Any,
	rkyv::Archive,
	rkyv::Serialize,
	rkyv::Deserialize,
)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune(constructor)]
pub struct PackageDefinitionEntity {
	/// The partition to add the path under in the packagedefinition file.
	pub partition: NonEmptyString,

	/// The path to add to packagedefinition. Generally, this is a platform-agnostic resource ID.
	pub path: NonEmptyString
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, JsonSchema, Clone, Debug, Type, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework, install_with = Self::rune_install)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
#[rune_functions(Self::r_new, Self::extend__meta)]
pub struct Localisation {
	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub english: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub french: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub italian: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub german: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub spanish: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub spanish_mexico: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub portuguese_brazil: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub turkish: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub polish: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub russian: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub chinese_simplified: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub chinese_traditional: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub japanese: Option<EcoString>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[schemars(with = "Option<String>")]
	#[specta(type = Option<String>)]
	pub korean: Option<EcoString>
}

impl Localisation {
	#[rune::function(path = Self::new)]
	fn r_new() -> Self {
		Self::default()
	}

	fn rune_install(module: &mut rune::Module) -> Result<(), rune::ContextError> {
		macro_rules! impl_field {
			($field:ident) => {
				module.field_function(
					&rune::runtime::Protocol::GET,
					stringify!($field),
					|s: &Self| -> Option<String> { s.$field.as_ref().map(|x| x.as_str().into()) }
				)?;

				module.field_function(
					&rune::runtime::Protocol::SET,
					stringify!($field),
					|s: &mut Self, v: Option<String>| {
						s.$field = v.map(|x| x.into());
					}
				)?;
			};
		}

		impl_field!(english);
		impl_field!(french);
		impl_field!(italian);
		impl_field!(german);
		impl_field!(spanish);
		impl_field!(spanish_mexico);
		impl_field!(portuguese_brazil);
		impl_field!(turkish);
		impl_field!(polish);
		impl_field!(russian);
		impl_field!(chinese_simplified);
		impl_field!(chinese_traditional);
		impl_field!(japanese);
		impl_field!(korean);

		Ok(())
	}
}

impl Localisation {
	/// Merge this Localisation with another Localisation, overwriting the current values with those from the other.
	#[rune::function(instance, keep, path = Self::extend)]
	pub fn extend(&mut self, other: Self) {
		if let Some(x) = other.english {
			self.english = Some(x);
		}

		if let Some(x) = other.french {
			self.french = Some(x);
		}

		if let Some(x) = other.italian {
			self.italian = Some(x);
		}

		if let Some(x) = other.german {
			self.german = Some(x);
		}

		if let Some(x) = other.spanish {
			self.spanish = Some(x);
		}

		if let Some(x) = other.spanish_mexico {
			self.spanish_mexico = Some(x);
		}

		if let Some(x) = other.portuguese_brazil {
			self.portuguese_brazil = Some(x);
		}

		if let Some(x) = other.turkish {
			self.turkish = Some(x);
		}

		if let Some(x) = other.polish {
			self.polish = Some(x);
		}

		if let Some(x) = other.russian {
			self.russian = Some(x);
		}

		if let Some(x) = other.chinese_simplified {
			self.chinese_simplified = Some(x);
		}

		if let Some(x) = other.chinese_traditional {
			self.chinese_traditional = Some(x);
		}

		if let Some(x) = other.japanese {
			self.japanese = Some(x);
		}

		if let Some(x) = other.korean {
			self.korean = Some(x);
		}
	}

	/// Get the first specified localisation string in (roughly) game order: English, French, Italian, German, Spanish, Russian, Spanish (Mexico), Portuguese (Brazil), Polish, Chinese (Simplified), Japanese, Chinese (Traditional), Korean, Turkish.
	pub fn first_specified(&self) -> Option<&EcoString> {
		self.english
			.as_ref()
			.or(self.french.as_ref())
			.or(self.italian.as_ref())
			.or(self.german.as_ref())
			.or(self.spanish.as_ref())
			.or(self.russian.as_ref())
			.or(self.spanish_mexico.as_ref())
			.or(self.portuguese_brazil.as_ref())
			.or(self.polish.as_ref())
			.or(self.chinese_simplified.as_ref())
			.or(self.japanese.as_ref())
			.or(self.chinese_traditional.as_ref())
			.or(self.korean.as_ref())
			.or(self.turkish.as_ref())
	}
}
