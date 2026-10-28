# texture.json

A game texture to add or overwrite. The metadata for the texture is in the form:

```json
{
    "text": "[assembly:/something.tex].pc_tex",
    "texd": "[assembly:/something.tex].pc_mipblock1", // Optional
    "type": "Colour",
    "format": "BC5", // Optional, defaults to BC7
    "interpretAs": "Height" // Optional, defaults to Normal
}
```

A file with the same name, but the extension `texture.dds` or `texture.png`, must be present next to the `texture.json` file and should contain the actual texture data.

Generally, DDS is the preferred format for a texture (as it does not require re-encoding), followed by PNG. However, DDS is a lossy format, meaning that each time a DDS file is saved, the quality of the texture becomes worse; thus, unless you have a separate source file (e.g. a PSD file) you should save textures you plan to edit multiple times in the PNG format.
