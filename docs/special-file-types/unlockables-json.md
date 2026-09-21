# unlockables.json

A JSON merge patch for the unlockables file. Entries are transformed so that the repository is a map from each item's `Id` to its value, rather than an array. For example:

```json
{
    "FIREARMS_HERO_PISTOL_KRUGERMEIER": {
        "Properties": {
            "RepositoryId": "c8a09c31-a53e-436f-8421-a4dc4115f633"
        }
    },
    "CUSTOM_ITEM_THAT_I_ADDED_WHICH_IS_ACTUALLY_THE_JAEGER_SNIPER": {
        "Id": "CUSTOM_ITEM_THAT_I_ADDED_WHICH_IS_ACTUALLY_THE_JAEGER_SNIPER",
        "Guid": "910asd56-0aea-4dac-93b7-a229faaoe24f",
        "Type": "weapon",
        "Subtype": "sniperrifle",
        "ImageId": "",
        "RMTPrice": 99,
        "GamePrice": 99,
        "IsPurchasable": false,
        "IsPublished": true,
        "IsDroppable": false,
        "Capabilities": [],
        "Qualities": {},
        "Properties": {
            "Gameplay": {
                "range": 1.0,
                "damage": 1.0,
                "clipsize": 0.2,
                "rateoffire": 0.3
            },
            "Name": "UI_FIREARMS_HERO_SNIPER_HEAVY_BASE_NAME",
            "Description": "UI_FIREARMS_HERO_SNIPER_HEAVY_BASE_DESC",
            "Quality": 4,
            "Rarity": "common",
            "LoadoutSlot": "carriedweapon",
            "RepositoryId": "370580fc-7fcf-47f8-b994-cebd279f69f9",
            "UnlockOrder": 5
        },
        "Rarity": "common"
    }
}
```

These values are merged into the existing unlockables file, so the first key will only overwrite the `Properties.RepositoryId` value rather than the entire existing unlockable entry.
