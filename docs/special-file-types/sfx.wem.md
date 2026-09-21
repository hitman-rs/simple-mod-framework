# sfx.wem

A Wwise WEM file to overwrite a specific entry in a WWEV file with.

The file must be named in the format `RuntimeID~wemID.sfx.wem`. Values after the next tilde are ignored by SMF, so you can add a comment for personal reference like `RuntimeID~wemID~gunshot.sfx.wem`. GlacierKit extracts WWEV files with this naming convention by default.
