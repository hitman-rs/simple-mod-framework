# localisation.patch.json

A patch to a LOCR file in the format:

```json
{
    "id": "[assembly:/something.sweetmenutext].pc_localized-textlist",
    "lines": {
        "english": {
            "UI_SOMETHING": "Something"
        }
    }
}
```

The same format for `lines` is used by manifest `localisation`; here, the lines are applied as a patch, so existing localisation for `UI_SOMETHING` will not be overridden in any language other than English.
