# Update checking

The `updateCheck` key has been removed. Instead, the `url` key which holds the mod's primary host is used to check for updates, and no longer requires a specific JSON format to be adhered to.

If your mod used the [SMF mod template](../mod-template.md), the URL will automatically be set to the same GitHub repository, allowing for the same benefits as in v2.

See [update-checking.md](../update-checking.md "mention") for more information on the new system.
