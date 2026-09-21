# JSON.patch.json

An RFC6902 JSON patch for a JSON resource (or the repository or unlockables files, which will be transformed to the format used by `repository.json` and `unlockables.json` files). Specified as:

```json
{
    "id": "[assembly:/some/file.json].pc_json", // The file to patch
    "patch": [{ // An RFC6902 format patch
        "op": "add",
        "path": "/Something/-",
        "value": {
            "a": "abc"
        }
    }]
}
```
