# What is a mod?

The Simple Mod Framework is a tool for deploying mods for HITMAN and 007 First Light. So what actually is a _mod_?

To SMF, a mod is a collection of source files which can be automatically converted, patched, or otherwise "baked" into their final form to be deployed as RPKG files to the game. This has a few key implications:

## No runtime behaviour

Anything that resides in the game's files can be modified by the Simple Mod Framework, but SMF cannot hook into the game dynamically.

{% hint style="info" %}
That's a job best suited for the **SDK** (for HITMAN 3: [ZHMModSDK](https://github.com/OrfeasZ/ZHMModSDK); for 007 First Light: [ZKntSdk](https://github.com/OrfeasZ/ZKntSDK/)). SMF allows you to bundle SDK mods (as DLL files) with your SMF mod, so users can still take advantage of any dynamic behaviour you want to add with little friction.
{% endhint %}

Likewise, SMF also cannot affect behaviour of the server, such as contracts mode, leaderboards, etc.

{% hint style="info" %}
Many people use a **server replacement** for the game to allow for offline, and more customisable, play. For HITMAN 3, this is [Peacock](https://github.com/thepeacockproject/Peacock). For 007 First Light, there is the work-in-progress [Octopussy](https://github.com/thepeacockproject/Octopussy) by the same team. Just like the SDK, SMF allows you to bundle plugins for Peacock with your mod. SMF also has [comprehensive integration](peacock-integration.md) with Peacock which automatically detects and handles important server-related features so that your mod functions largely the same on Peacock as it does in offline mode.
{% endhint %}

The lines are often blurred when it comes to what requires this sort of dynamic behaviour - entities can do a lot, but a lot is also hard-coded into the game engine, or decided by the server. For SMF, the question comes down to:

> Does this functionality reside in resources contained in the RPKG files?

If so, SMF can patch it. If not, you'll want to reach for the SDK or Peacock - but SMF will still help you distribute and manage the result.

## Mod compatibility

The Glacier engine is highly moddable, but that doesn't mean it was _designed_ to be modded. A patch RPKG file contains the full contents of game resources in their final packed form, so mods made in this form are simply not compatible with anything that affects the same resources. Many resources contain global metadata or lists that many mods will want to change, so SMF makes this possible by **patching**.

SMF deploys mods in the enabled list from top to bottom, with later mods overwriting files from earlier ones. But most of the time, this doesn't actually matter, because SMF handles many files (like `entity.patch.json` files) by applying a series of changes rather than overwriting the entire resource. This also helps with game updates - when IO Interactive changes a file, you don't need to worry unless they changed the part you care about.

SMF will help you make sure that your mods are compatible with other mods, and with game updates, by warning you of potentially incompatible files and, in some cases, automatically converting them to patches.
