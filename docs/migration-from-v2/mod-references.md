# Mod references

In SMFv2, other mods were referenced by keys like `requirements` by their IDs. SMFv3 now enforces that a version range is provided, usually in the form of a simple latest version (e.g. `Notex.PortableChair@2.0.0`).

This requirement has been introduced to ensure that mods are never negatively affected when other authors do or don't update their mods. For example, if a mod contains an `incompatibleMods` key which references your mod, but you have updated your mod to fix the incompatibility, you can now (and **must** according to SMF's system of [Semantic Versioning](https://semver.org/)) increment the major version, causing the `incompatibleMods` entry to no longer affect your mod.
