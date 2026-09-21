# repository.json

A JSON merge patch for the repository file. Entries are transformed so that the repository is a map from each item's `ID_` to its value, rather than an array. For example:

```json
{
    "7a714602-2103-4271-9766-233b9e2154db": {
        "Image": "images/customImages/formerPrimeMinisterOfAustralia.jpg",
        "Name": "Kevin Rudd"
    },
    "7a62219e-008a-4a0a-b233-768d39287842": {
        "ID_": "7a62219e-008a-4a0a-b233-768d39287842",
        "Image": "images/actors/actor_a166a37e-a3f8-42d2-99d6-e0dd2cf5c090_1_0_0.jpg",
        "Name": "Thisguy's Adeadman",
        "Outfit": "a166a37e-a3f8-42d2-99d6-e0dd2cf5c090",
        "OutfitVariationIndex": 1.0,
        "CharacterSetIndex": 0.0,
        "Description": "Unknown",
        "Description_LOC": "actor_description",
        "Tile": "images/actors/default_target.png"
    }
}
```

These values are merged into the existing repository, so the first key will only overwrite the `Image` and `Name` values rather than the entire existing repository entry.
