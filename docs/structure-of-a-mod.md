# Structure of a mod

A mod, as uploaded to a site like Nexus Mods, is a ZIP or 7-Zip archive of the mod's files, which SMF extracts as part of the process when a user clicks Add Mod. The mod's folder, then, can contain practically anything, so long as there is a `manifest.json` at its root. All the other important files and folders of a mod are referenced by the manifest.

In general, a mod therefore looks something like this:

```
Mods
-> Atampy26.RealisticAI
   -> content
   -> manifest.json
```

When it comes time to distribute the mod, you simply ZIP up all the files inside your mod folder so that `manifest.json` is contained at the immediate "top level" of the ZIP.

