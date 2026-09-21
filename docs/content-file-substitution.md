# Content file substitution

All textual files included in content folders are searched for keywords to find-and-replace, which can provide a convenient way of using an option value without having to create multiple copies of a file. For example, the value of a boolean option can be replicated in a `ValueBool` game entity for use at runtime, or a provided colour can be directly used as the diffuse colour of an outfit.

## Options

Mod options can be substituted into any content file by their ID, using `#{option:some-option}` or `#{option-raw:some-option}`. The latter will always be replaced directly with the value, while the former will be more "intelligently" replaced so that for non-string values, wrapping quotes will be removed. For example,

```json
{
    "something": "#{option:some-option}",
    "something2": "bla #{option:some-option} bla",
    "something3": "#{option-raw:some-option}"
}
```

will become (assuming the value of `some-option` is 5):

```json
{
    "something": 5,
    "something2": "bla 5 bla",
    "something3": "5"
}
```

### Option values

Each of the option types has some associated value for substitution:

* **Boolean, number, color, string**: the option value
* **Selection**: the selection group itself is substituted as the selected ID, while each sub-option is substituted as `true` or `false` for whether that sub-option is selected
* **Conditional, option group**: `true` or `false` for whether the condition is met

## Scripts

Scripts can emit their own substitutions as part of the `data` function, which returns an array of `(string, any)` pairs corresponding to the ID and value of each substitution respectively. Like options, these can be used with `#{script:substitution-id}` or `#{script-raw:substitution-id}`, which behave the same as the above with respect to the type of the value.
