# contract.json

A contract JSON. The contract inside is automatically given a hash/path and added to the contracts ORES. Existing game contracts can also be edited, and will simply be overwritten.

The top-level extension key `SMF` can be used to control certain aspects of how the contract is deployed:

```json
{
    "SMF": {
        "destinations": {
            "addToDestinations": true, // Whether the contract should automatically be added to the Destinations page
            "peacockIntegration": true, // Whether Peacock should respect these settings, defaults to true if omitted
            "narrativeContext": "Mission", // Mission or Campaign in the base game, defaults to Mission if omitted
            "placeBefore": "735d005c-698e-5a3f-9a55-15e4fea0f816", // A contract ID to place this one before; cannot be used with placeAfter
            "placeAfter": "735d005c-698e-5a3f-9a55-15e4fea0f816" // A contract ID to place this one after; cannot be used with placeBefore
        }
    }
}
```
