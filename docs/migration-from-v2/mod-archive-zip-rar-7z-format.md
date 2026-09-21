# Mod archive (ZIP/RAR/7z) format

Previously, mods contained a root folder within the archive file which itself contained the manifest. This folder would essentially be copied into the SMF Mods folder. SMFv3 instead expects mods to contain the manifest directly, with SMF automatically naming the folder in Mods according to the mod's ID.

Additionally, support for the RAR format has been removed altogether. RAR was a substantially inferior format for mod distribution than ZIP and 7z, with worse usability than ZIP and worse compression than 7z. You should distribute your mod as a ZIP file, or if the file is large and would benefit from compression, a 7z file.
