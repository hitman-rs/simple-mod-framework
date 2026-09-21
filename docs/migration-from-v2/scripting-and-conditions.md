# Scripting and conditions

The scripting system has been entirely revamped, and now uses the [Rune language](https://rune-rs.github.io/), as well as following a significantly different mental model. Mods with scripts cannot be automatically upgraded from v2 to v3, so you will need to remove these before migrating your mod.

Conditions, which previously used the Filtrex language, now also use Rune expressions. These are far more common in existing mods than scripts, and the framework can automatically upgrade a number of common condition styles, such as `"Author.Mod" in config.loadOrder`, to ease this change.

As part of the move to Rune, scripts are now sandboxed. As such, they cannot alter the filesystem or inspect anything beyond what the framework specifically allows. Thus, scripts no longer give a warning to users regarding security.
