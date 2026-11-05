# SDK integration

SMF allows users to manage their installation of the SDK (for HITMAN 3: [ZHMModSDK](https://github.com/OrfeasZ/ZHMModSDK); for 007 First Light: [ZKntSdk](https://github.com/OrfeasZ/ZKntSDK/)) in the Settings page, and allows SMF mods to bundle SDK mods (as DLLs) with the `sdkMods` manifest key.

{% hint style="warning" %}
Note that SDK mods carry security concerns (as they can run arbitrary code), so SMF will display a warning to users if your mod contains SDK mods.
{% endhint %}

SMF does not currently automatically alter the SDK configuration, so users will still need to enable the deployed mod with the SDK's in-game menu.
