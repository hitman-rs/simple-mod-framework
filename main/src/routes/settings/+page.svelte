<script lang="ts">
	import { config, saveConfig } from "$lib/config.svelte"
	import * as m from "$lib/paraglide/messages"
	import { locales } from "$lib/paraglide/runtime"
	import SDKManager from "./SDKManager.svelte"

	const localeNames = {
		en: "English",
		es: "Español",
		"es-MX": "Español (México)",
		fr: "Français",
		de: "Deutsch",
		it: "Italiano",
		ja: "日本語",
		"zh-Hans": "简体中文",
		"zh-Hant": "繁體中文",
		pl: "Polski",
		"pt-BR": "Português (Brasil)",
		ru: "Русский",
		tr: "Türkçe",
		ko: "한국어"
	}
</script>

<div class="mb-3">
	<div class="mb-1">{m.Language()}</div>
	<sl-select
		class="max-w-md"
		placeholder={m.Language()}
		value={config.uiLocale}
		onsl-change={async (e) => {
			config.uiLocale = e.target.value
			await saveConfig()
			window.location.reload()
		}}
	>
		{#each locales as locale}
			<sl-option value={locale}>{localeNames[locale]}</sl-option>
		{/each}
	</sl-select>
</div>
<sl-checkbox
	checked={config.skipIntro}
	onsl-change={(evt) => {
		config.skipIntro = evt.target.checked
	}}>{m.SkipIntro()}</sl-checkbox
>
<br />
<sl-tooltip content={m.ObeyDynresDisableDesc()} placement="right">
	<sl-checkbox
		class="mt-2"
		checked={config.autoDisableDynres}
		onsl-change={(evt) => {
			config.autoDisableDynres = evt.target.checked
		}}>{m.ObeyDynresDisable()}</sl-checkbox
	>
</sl-tooltip>
<br />
<sl-tooltip content={m.DeveloperModeUpsellDesc()} placement="right">
	<sl-checkbox
		class="mt-2"
		checked={config.developerMode}
		onsl-change={(evt) => {
			config.developerMode = evt.target.checked
		}}>{m.EnableDeveloperMode()}</sl-checkbox
	>
</sl-tooltip>
<br />
<sl-tooltip content={m.EnableOnlineServicesDesc()} placement="right">
	<sl-checkbox
		class="mt-2"
		checked={config.onlineServices}
		onsl-change={(evt) => {
			config.onlineServices = evt.target.checked
		}}>{m.EnableOnlineServices()}</sl-checkbox
	>
</sl-tooltip>
<br />
<div class="mt-2">
	<sl-tooltip content={m.CustomBootSceneDesc()} placement="right">
		<sl-checkbox
			checked={config.bootScene !== null}
			onsl-change={(evt) => {
				if (evt.target.checked) {
					config.bootScene = ""
				} else {
					config.bootScene = null
				}
			}}>{m.CustomBootScene()}</sl-checkbox
		>
	</sl-tooltip>
	{#if config.bootScene !== null}
		<br />
		<sl-input
			class="mt-2"
			placeholder="assembly:/_pro/scenes/frontend/boot.entity"
			value={config.bootScene}
			onsl-change={(e) => {
				config.bootScene = e.target.value
			}}
		></sl-input>
	{/if}
</div>
<br />
<div>
	<div class="mb-2 font-bold">Mod SDK</div>
	<p class="mb-4">
		{m.ZHMModSDKDesc()}
		<br />
		{m.ZHMModSDKSafetyDesc()}
	</p>
	<SDKManager />
</div>
