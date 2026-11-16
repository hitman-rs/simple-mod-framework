# Custom enum variants

The framework supports defining custom enum variants, which can be used by entities or an SDK mod to add new functionality where an enum would otherwise constrain modding.

The values for these variants are emitted in the deploy summary under the `enums` key, which is a map from each enum name's to a map of variants (qualified as `ModID:VariantName`) to their values.

## Extension of existing enums

You may add new variants to an existing game enum; the integer value of these will be automatically chosen from the available values of that enum's size, and will not conflict with any vanilla enum value or any variant defined by an other mod.

## "Custom" enums

You can also specify enums that do not exist in the vanilla game. These will have their size assumed to be 4 bytes (an `int32`) and otherwise will be treated as above. **This does not make your custom enum available as a type in entities** (this is not possible), so you should use the `int32` type.

## Value assignment

The potential values of an enum variant depend on the type of the variant. The default is `int` - this will be used if a plain string is supplied in the manifest.

### Int variants

An `int` variant represents a named integer value; the vast majority of enums use these.

**No guarantee is made of what value these will be assigned.** Any value that is valid for the enum's type/size and is not already taken by another variant (vanilla or modded) may be chosen.

### Bit variants

A `bit` variant represents a bit index: that is, a value between 0 and the highest bit of the type. For example, for a 32-bit enum the highest `bit` variant value would be 31. This is useful in cases where the game performs a shift to determine a mask (i.e. by doing `1 << enum_value`).

Any value within bounds not already taken by another variant may be chosen.

### Flag variants

A `flag` variant represents a single bitflag. In numeric terms, this means the value will be a power of two; represented in binary, a single bit will be set on the value. There are as many valid flag variants for a given enum as there are bits of the enum.

Any power of two not already taken by another variant may be chosen.

### Flags variants

A `flags` variant represents a combination of `flag` values (so, a value where multiple flags are set). The value assigned will be the logical OR of all the variants specified in `flags`. These variants are permitted to collide in their assigned value since they do not represent "separate" variants.
