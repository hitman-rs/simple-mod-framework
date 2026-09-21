# Peacock integration

Peacock and SMF are highly integrated, with the goal that your mod should function the same in Peacock as it would in offline mode.

## Contracts

Contracts defined in `contract.json` files are automatically registered with Peacock, and if the `addToDestinations` flag is set, this will be applied to Peacock's destinations menu as well.

## Unlockables

The unlockables ORES file is automatically used instead of Peacock's default if it has been modified.

Additionally, new locations (and changes to existing locations) defined in unlockables are automatically added to the appropriate Peacock configuration.

## Entrances and Agency Pickups

World map metadata JSON files referenced by `[assembly:/templates/ui/mapexportentities.template?/menumap.entitytemplate].pc_entitytype` files are automatically detected by SMF and passed to Peacock, which automatically configures the `Entrances` and `AgencyPickups` configurations as necessary.

## Campaigns

Patches to the game's `storyconfig` file to add or modify campaigns are automatically detected and replicated in Peacock's campaign configuration.

## Plugins

If you wish to add additional functionality, like challenges, you can bundle Peacock plugins with your mod and reference them in the [manifest](manifest-format.md#manifestdata). Peacock will automatically load them after a user has deployed the mod.
