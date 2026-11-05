# Custom enum variants

The framework supports defining custom enum variants, which can be used by entities or an SDK mod to add new functionality where an enum would otherwise constrain modding.

The values for these variants are emitted in the deploy summary under the `enums` key, which is a map from each enum name's to a map of variants (qualified as `ModID:VariantName`) to their values.

## Extension of existing enums

You may add new variants to an existing game enum; the integer value of these will be automatically chosen from the available values of that enum's size, and will not conflict with any vanilla enum value or any variant defined by an other mod.

## "Custom" enums

You can also specify enums that do not exist in the vanilla game. These will have their size assumed to be 4 bytes (an `int32`) and otherwise will be treated as above. **This does not make your custom enum available as a type in entities** (this is not possible), so you should use the `int32` type.

## Value assignment

**No guarantee is made of what value an enum variant will be assigned.** Any value that is valid for the enum's type/size and is not already taken by another variant (vanilla or modded) may be chosen.
