# Update checking

SMF automatically checks for updates whenever possible based on the mod's `url` property, as specified in the [manifest](manifest-format.md#manifest).

For SMF to check updates, your mod must use one of the following for its `url`:

* The [GitHub](https://github.com/) repository, e.g. [https://github.com/NotexMods/Portable-Chair](https://github.com/NotexMods/Portable-Chair)
* The [ModWorkshop](https://modworkshop.net) mod page, e.g. [https://modworkshop.net/mod/45444](https://modworkshop.net/mod/45444)
* The [Nexus Mods](https://nexusmods.com) mod page, e.g. [https://www.nexusmods.com/hitman3/mods/453](https://www.nexusmods.com/hitman3/mods/453)

Your mod may have multiple of these - all of the above examples point to Notex's Portable Chair mod. You should use the one which supports the most features:

<table><thead><tr><th width="187">Feature</th><th width="137" data-type="checkbox">GitHub</th><th width="151" data-type="checkbox">ModWorkshop</th><th width="145" data-type="checkbox">Nexus Mods</th><th data-type="checkbox">Other hosts</th></tr></thead><tbody><tr><td><a data-footnote-ref href="#user-content-fn-1">Update checking</a></td><td>true</td><td>true</td><td>true</td><td>false</td></tr><tr><td><a data-footnote-ref href="#user-content-fn-2">Changelogs</a></td><td>true</td><td>false</td><td>false</td><td>false</td></tr><tr><td><a data-footnote-ref href="#user-content-fn-3">Automatic updates</a></td><td>true</td><td>true</td><td>false</td><td>false</td></tr></tbody></table>

For the optimal experience, you should prefer a **GitHub** link if possible, followed by ModWorkshop and lastly Nexus Mods.

Currently, SMF allows other URLs not from any host above to be provided, as long as they use HTTPS. **This is not guaranteed to stay the case, even between patch versions. The available hosts may be restricted in future updates.**

[^1]: SMF can tell you if a newer version of the mod is available.

[^2]: SMF can show you a description of what's changed between the currently installed version and the latest one.

[^3]: Automatically download and update the mod by clicking a button in SMF.
