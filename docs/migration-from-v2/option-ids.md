# Option IDs

The format of mod options has been significantly changed, and in particular, all options must now specify a unique ID which the framework, your mod, and other mods will use to refer to them.

The convention for an option ID is `kebab-case`. SMF's auto-upgrade process automatically gives every option its own ID in the format `change-me-to-something-else-<hash of name>`. In upgrading your mod, you should change these IDs to intuitive, human-readable identifiers - users will see these IDs, and other mod authors may need to reference them.
