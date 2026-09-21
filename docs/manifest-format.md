# Manifest format

Every mod must contain a JSON manifest describing the mod. This documentation is taken from the schema, which you can embed with the `$schema` JSON property to get autocompletion and documentation inline as you write your manifest in an editor like VS Code. GlacierKit automatically applies the schema, regardless of the `$schema` property.

## Manifest

_The root mod manifest object._

**Properties**

* `id` (string)
  * The mod's ID. Should follow capitalised reverse URI style (AuthorName.ModName). Don't include special characters; numbers are OK. Words should be separated by CamelCase.
* `name` (UIText)
  * The name of the mod.
* `description` (UIText)
  * A description of the mod.
* `authors` (array of string)
  * A list of the mod's authors.
* `version` (string)
  * The mod's version, using [semantic versioning](https://semver.org/) (X.Y.Z). As applied to mods:
    * updates with only bug fixes/patches increment the third number (patch version)
    * updates with any new features increment the second number (minor version)
    * updates which change the mod's behaviour in relation to other mods increment the first number (major version)
  * The major version should almost never be changed if the mod has not massively changed, but any change which affects other mods (for example, in compatibility or incompatibility) MUST increment it. For example, if the new version makes your mod newly compatible with another mod, the major version should be incremented so that any `incompatibilities` keys from other mods no longer affect yours.
* `frameworkVersion` (string)
  * The earliest version of the framework that this specific version of the mod has been tested against.
* `url` (string)
  * A link to the mod's primary host (must be HTTPS).
    * A mod hosted on Nexus should link to the Nexus page (e.g. https://nexusmods.com/hitman3/mods/453 - you can find the link before releasing the mod by using the preview function).
    * A mod hosted on GitHub should use the GitHub repository link (https://github.com/Notexe/Portable-Chair).
    * ModWorkshop mods should use their link (https://modworkshop.net/mod/45444).
  * Each platform has differing support for SMF features.
    * GitHub repositories and ModWorkshop mods support fully automatic updating.
    * GitHub repositories additionally support proper changelogs for mods, so users can see all the differences (across all releases) between their version and the latest. ModWorkshop does not support changelogs.
    * Nexus Mods only allows for telling the user when an update is available, so the other two platforms are preferable.
  * In short: use GitHub if you can, ModWorkshop if you really don't want to, and Nexus Mods as a last resort.
* `links` (Links)
  * Relevant links for the mod.
* `conditions` (ManifestConditions)
  * The `supportedGames` key is required for the top-level manifest.
* `data` (ManifestData, optional)
* `options` (array of ModOption, optional)
  * Settings for the mod that can be customised in the Mod Manager.
* `presets` (array of OptionPreset, optional)
  * Preset combinations of mod options that can be selected all at once.

## UIText

_A line of UI text, which can be localised into any of the framework's supported languages._

This type has two forms. The first is a single line of text:

```json
"description": "A description."
```

This is interpreted as a line of **English** text, with no localisation into other languages. You can also supply an object with any of the framework's supported languages:

```json
"description": {
    "english": "A description.",
    "french": "Une description."
}
```

The supported languages are the same as those of the World of Assassination games and 007 First Light:

* `english` (en)
* `french` (fr)
* `italian` (it)
* `german` (de)
* `spanish` (es)
* `spanishMexico` (es-MX)
* `portugueseBrazil` (pt-BR)
* `turkish` (tr)
* `polish` (pl)
* `russian` (ru)
* `chineseSimplified` (zh-Hans)
* `chineseTraditional` (zh-Hant)
* `japanese` (ja)
* `korean` (ko)

## Links

_Links to related websites other than the mod's homepage/primary host._

**Properties**

* `info` (string, optional)
  * A link to documentation about the mod, separate to its host/homepage (e.g. mod wiki).
* `issues` (string, optional)
  * A link to report issues with the mod (e.g. Nexus comments section, GitHub Issues page).
* `support` (string, optional)
  * A link to support the mod's development (e.g. Patreon, Ko-fi, GitHub Sponsors).

## ManifestConditions

_Conditions required for a mod to be deployed or for an option to be configurable._

**Properties**

* `supportedGames` (array of string, optional)
  * Games that this mod supports. All other games will be considered unsupported by this mod.
  * You can specify entire game versions (e.g. `h1`) or specific version/platform combinations (e.g. `h3-epic`).
  * Use this when a mod uses features that only some games support, such as Ghost Mode and H2 (plus H3 Steam).
* `requiredMods` (array of ModReference, optional)
  * Mods that this mod depends on to function. Clients without these mods enabled will be prevented from using this mod.
* `requiredConditions` (array of ExplainedCondition, optional)
  * Conditions which this mod depends on to function. When any condition is not met, the user will be prevented from using this mod.
* `incompatibleMods` (array of ModReference, optional)
  * Mods that this mod will not function with. Clients with these mods enabled will be prevented from using this mod.
* `incompatibleConditions` (array of ExplainedCondition, optional)
  * Conditions which this mod is incompatible with. When any condition is met, the user will be prevented from using this mod.

## ModReference

_A reference to a mod's ID accompanied by a version range._

Mods are referenced by their ID. To ensure that mods can reliably depend on each other, a version is also required. This should be specified as `modID@version`, where `version` can be a simple version (e.g. `1.0.0`, meaning 1.x.x) or a range specifier in standard npm-style syntax (e.g. `^2.0.0` or `1.2.0-1.3.1`).

## ExplainedCondition

_A script condition accompanied by an explanation._

**Properties**

* `condition` (string)
  * A condition written in Rune. Should be formatted as an expression. Some helper functions are given in the global scope. The framework config and game (version and platform) are available in the global scope as `config` and `game`.
  * For example, `mod_option("Author.SomeMod@1.0.0", "an-option") == Some("a-value")`.
* `explanation` (UIText)
  * A short explanation of the condition, to be shown to the user when the condition is not met. For example, "Incompatible with Lighting Ultimate's Vanilla+ Sapienza" or "Requires either Mod A or Mod B to be enabled". Should not end with punctuation.

## ManifestData

_Content and information that makes up a mod or option._

**Properties**

* `contentFolders` (array of Path)
  * Folders with content files that will be crawled and automatically deployed.
* `blobFolders` (array of Path)
  * Folders with blobs that will be crawled and automatically deployed.
* `localisation` (map of string -> Localisation)
  * Localisation keys (and their text values) to make globally available.
  * **Localisation**
    *   An object in the form:

        ```json
        {
            "english": "Text",
            "french": "Texte"
        }
        ```
    * Supports the same languages as the framework. Languages which are not supported by the current game version will simply be ignored.
* `localisedLines` (map of RuntimeID -> string)
  * LINE files to create from localisation keys specified in `localisation`.
* `packageDefinition` (array of PackageDefinitionEntity)
  * Paths to add to packagedefinition. Custom partitions are not supported.
  * **PackageDefinitionEntity**
    * `partition` (string)
      * The partition to add the path under in the packagedefinition file.
    * `path` (string)
      * The path to add to packagedefinition. Generally, this is a platform-agnostic resource ID.
* `portResources` (array of PortedResource)
  * Resources that will be made available to a given partition (or super/chunk0 if unspecified).
  * **PortedResource**
    * Can be specified either as a string or object.
    * As a string: the ID of the resource to port. The resource will be made available to `super` (and thus to all partitions).
    * As an object: `{ resource: RuntimeID, forPartition: string }` where the resource will be made available to the specified partition.
* `deployBefore` (array of ModReference)
  * Mods that this mod should deploy before. Used in automatic sorting by the Mod Manager.
* `deployAfter` (array of ModReference)
  * Mods that this mod should deploy after. Used in automatic sorting by the Mod Manager.
* `peacockPlugins` (array of Path)
  * Paths to plugins that Peacock should load when this mod is deployed.
* `sdkMods` (map of GlacierGame -> array of Path)
  * Game versions, and the paths to mod DLLs that the appropriate SDK (ZHMModSDK/ZKntSDK) should load when this mod is deployed.
  * **GlacierGame**: one of `h1`, `h2`, `h3`, `fl`.
* `scripts` (array of Path)
  * Paths to Rune files that can alter deployment of the mod. The scripting API is currently unstable and may change between framework versions.

## Path

Paths are represented as strings containing the relative path within the mod folder, i.e. the path from next to the manifest. They must use forward slashes (`/`) as the folder separator, and are case-sensitive (even on Windows, where paths are normally case-insensitive). Paths may not escape the mod folder (e.g. `../AnotherMod/something`).

## ModOption

_An option that the user can configure in the SMF app._

**Properties**

* `id` (string)
  * An ID for the option. Must not be duplicated. The convention used for mod option IDs is `kebab-case`.
  * This is also used for [substitution](content-file-substitution.md#options) in content files; `"bla": "#{option:some-option}"` will be replaced with `"bla": true` and `"bla #{option:some-option} bla"` with `"bla true bla"`.
* `type` (string)
  * The type of the option.
  * One of: `boolean`, `selection`, `conditional`, `number`, `color`, `string`, `optionGroup`.

### Boolean

_A boolean option. Can be enabled or disabled by a user._

**Additional properties**

* `name` (UIText)
  * The name of the option.
* `description` (UIText, optional)
  * A description of the option. Can contain multiple sentences.
* `defaultValue` (boolean)
  * The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met). Preselected options should deliver the "advertised experience". Mods with many standalone options should have the user select their own settings, or provide a conservative default.
* `image` (Path, optional)
  * An image representing the option. Should usually be a gameplay image showing the option in use.
* `conditions` (ManifestConditions, optional)
  * Conditions which must be met for this option to be configurable.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option's value is true.

### Selection

_A selection group, where the user selects only one option to enable from a list of options._

**Additional properties**

* `name` (UIText)
  * The name of the selection group.
* `description` (UIText, optional)
  * A description of the selection group. Can contain multiple sentences.
* `defaultValue` (ModOptionID)
  * The default-selected option's ID. Will also be used if the selection group itself is disabled (e.g. if it is part of a group whose `displayCondition` is not met).
* `conditions` (ManifestConditions, optional)
  * Conditions which must be met for this option to be configurable.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option is configurable (when all conditions are met).
* `options` (array of SelectionOption)
  * The available options to choose from.
  * **SelectionOption**
    * `id` (ModOptionID)
      * An ID for the sub-option. Must also be unique including all other options in the mod.
    * `name` (UIText)
      * The option's name.
    * `description` (UIText, optional)
      * A description of the option. Can contain multiple sentences.
    * `image` (Path, optional)
      * An image representing the option. Should usually be a gameplay image showing the option in use.
    * `conditions` (ManifestConditions, optional)
      * Conditions which must be met for this option to be selectable.
    * `data` (ManifestData, optional)

### Conditional

_A conditional option. This is a special case; conditional options are not shown to the user, and are instead enabled if and only if the given condition is met._

**Additional properties**

* `condition` (string)
  * A condition written in Rune which will enable the option when met. Should be formatted as an expression. Some helper functions are given in the global scope. The framework config and game (version and platform) are available in the global scope as `config` and `game`.
  * For example, `mod_enabled("Author.SomeMod@1.0.0") && game.platform == Platform::Steam`.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option's condition evaluates to true.

### Number

_A number option. The user can enter a number (optionally with validation)._

**Additional properties**

* `name` (UIText)
  * The name of the option.
* `description` (UIText, optional)
  * A description of the option. Can contain multiple sentences.
* `defaultValue` (number)
  * The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
* `validation` (NumberOptionValidation)
  * Validation settings for this option.
  * **NumberOptionValidation**
    * `minimum` (number, optional)
      * The minimum value the user is allowed to enter.
    * `maximum` (number, optional)
      * The minimum value the user is allowed to enter.
    * `integer` (boolean, optional)
      * Whether the user must enter an integer (a whole number).
* `image` (Path, optional)
  * An image representing the option. Should usually be a gameplay image showing what effect the option might have.
* `conditions` (ManifestConditions, optional)
  * Conditions which must be met for this option to be configurable.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option is configurable (when all conditions are met).

### Color

_A colour option. The user can select a colour using a visual picker._

**Additional properties**

* `name` (UIText)
  * The name of the option.
* `description` (UIText, optional)
  * A description of the option. Can contain multiple sentences.
* `defaultValue` (string)
  * The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
* `alpha` (boolean, optional)
  * Whether the colour should include a transparency/alpha value. If disabled (default), the value of this option is of the form "#rrggbb". If enabled, it is "#rrggbbaa".
* `image` (Path, optional)
  * An image representing the option. Should usually be a gameplay image showing what effect the option might have.
* `conditions` (ManifestConditions, optional)
  * Conditions which must be met for this option to be configurable.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option is configurable (when all conditions are met).

### String

_A string option. The user can enter a freeform string (optionally with validation)._

**Additional properties**

* `name` (UIText)
  * The name of the option.
* `description` (UIText, optional)
  * A description of the option. Can contain multiple sentences.
* `defaultValue` (string)
  * The default value of this option. Will also be used if the option is disabled (e.g. if this option is part of a group whose `displayCondition` is not met).
* `validation` (StringOptionValidation)
  * Validation settings for this option.
  * **StringOptionValidation**
    * `pattern` (string, optional)
      * A regular expression that the user's input must match. Should be a valid Rust regex. For example, `^[a-zA-Z0-9]+$` to require a non-empty string with only alphanumeric characters.
    * `minLength` (number, optional)
      * The minimum length of the user's input.
    * `maxLength` (number, optional)
      * The maximum length of the user's input.
* `image` (Path, optional)
  * An image representing the option. Should usually be a gameplay image showing what effect the option might have.
* `conditions` (ManifestConditions, optional)
  * Conditions which must be met for this option to be configurable.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option is configurable (when all conditions are met).

### Option Group

_A group of related options, for visual formatting._

**Additional properties**

* `displayCondition` (string, optional)
  * This entire option group will only be displayed (and only be configurable) if the given condition is met. Should be formatted as an expression; see the documentation for examples.
* `name` (UIText)
  * The name of the group.
* `description` (UIText, optional)
  * A description of the group. Can contain multiple sentences.
* `hiddenDescription` (UIText, optional)
  * A description of the group, to be shown when the display condition is NOT met. Can contain multiple sentences.
  * If not provided, the group will be hidden altogether when the display condition is not met.
* `options` (array of ModOption)
  * The options contained within this group.
* `presets` (array of OptionPreset, optional)
  * Preset combinations of mod options that can be selected all at once.
* `data` (ManifestData, optional)
  * Data to conditionally deploy when this option group is displayed (when its condition is met).
