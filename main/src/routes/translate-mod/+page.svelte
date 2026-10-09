<script lang="ts">
	import { page } from "$app/state"
	import type { UIText } from "$lib/bindings"
	import { config, saveConfig } from "$lib/config.svelte"
	import { getModManifest, linesToTranslate, loc, localeSmf } from "$lib/mods.svelte"
	import { m } from "$lib/paraglide/messages"
	import { getLocale, locales } from "$lib/paraglide/runtime"
	import { localeNames } from "$lib/utils"
	import { JsonPointer } from "@croct/json-pointer"
	import { save } from "@tauri-apps/plugin-dialog"
	import { writeTextFile } from "@tauri-apps/plugin-fs"

	const mod = page.url.searchParams.get("mod")!
	const manifest = $derived(getModManifest(mod))

	const translations = $state({})
	let sourceLocales = $state(["en"])

	let savedDialog: any
</script>

<sl-dialog label={m.TranslationsSaved()} bind:this={savedDialog}>
	{m.TranslationsSavedDesc()}
</sl-dialog>

{#await manifest then manifest}
	{@const toTranslate = linesToTranslate(manifest, getLocale())}
	<h1 class="text-4xl 2xl:text-5xl font-bold mb-2">{manifest.name}</h1>
	<div class="flex flex-row flex-wrap gap-2 mb-4">
		<sl-select
			label={m.SourceLanguages()}
			multiple
			clearable
			value={sourceLocales.join(" ")}
			onsl-change={(evt) => {
				sourceLocales = evt.target.value
			}}
		>
			{#each locales as locale}
				<sl-option value={locale}>{localeNames[locale]}</sl-option>
			{/each}
		</sl-select>
		<sl-select
			label={m.TargetLanguage()}
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
		<div class="pt-[1.6em]">
			<sl-button
				variant="success"
				onclick={async () => {
					const patch = Object.entries(translations)
						.filter((a) => a[1] !== "")
						.map(([path, value]) => {
							if (typeof JsonPointer.parse(path).get(manifest as any) === "string") {
								return { op: "replace", path: `${path}`, value: { english: JsonPointer.parse(path).get(manifest as any), [localeSmf[getLocale()]]: value } }
							} else {
								return { op: "add", path: `${path}/${localeSmf[getLocale()]}`, value }
							}
						})

					const path = await save({
						title: "Save translation file",
						filters: [{ name: "JSON patch", extensions: ["json"] }],
						defaultPath: "translations.json"
					})

					if (path) {
						await writeTextFile(path, JSON.stringify(patch, null, "\t"))
						savedDialog.show()
					}
				}}>{m.SaveTranslationsButton()}</sl-button
			>
		</div>
	</div>
	<div class="grid grid-cols-2 items-center gap-x-8 gap-y-4 max-h-[calc(100vh-14rem)] overflow-y-auto overflow-x-hidden">
		{#each toTranslate as path}
			{@const value = JsonPointer.parse(path).get(manifest as any) as UIText}
			{@const languages = typeof value === "string" ? { english: value } : value}
			<div>
				<div class="text-neutral-300 text-sm break-all mb-1"><code>{path}</code></div>
				<div class="grid gap-x-3 gap-y-1" style="grid-template-columns: auto 1fr;">
					{#each sourceLocales as locale}
						{#if typeof languages[localeSmf[locale]] !== "undefined"}
							<sl-badge>{localeNames[locale]}</sl-badge>
							<div>{loc(value, locale)}</div>
						{/if}
					{/each}
				</div>
				{#if !sourceLocales.some((locale) => typeof languages[localeSmf[locale]] !== "undefined")}
					<div class="grid gap-x-3 gap-y-1" style="grid-template-columns: auto 1fr;">
						{#each Object.entries(languages) as [lang, text]}
							<sl-badge>{localeNames[Object.entries(localeSmf).find((a) => a[1] === lang)![0]]}</sl-badge>
							<div>{text || loc(value)}</div>
						{/each}
					</div>
				{/if}
			</div>
			<div class="grid gap-3" style="grid-template-columns: auto 1fr auto;">
				<sl-badge>{localeNames[getLocale()]}</sl-badge>
				{#if translations[path] !== null}
					<sl-textarea class="w-full" rows={1} resize="auto" value={translations[path] || ""} oninput={(evt) => (translations[path] = evt.target.value)}></sl-textarea>
					<sl-tooltip content={m.MarkNoTranslate()}>
						<sl-button
							circle
							variant="primary"
							href="#"
							onclick={() => {
								translations[path] = null
							}}
						>
							<sl-icon name="x-lg" label={m.MarkNoTranslate()}></sl-icon>
						</sl-button>
					</sl-tooltip>
				{:else}
					<div class="text-neutral-300 italic self-center">{loc(value)}</div>
					<sl-tooltip content={m.UnmarkNoTranslate()}>
						<sl-button
							circle
							variant="primary"
							href="#"
							onclick={() => {
								translations[path] = ""
							}}
						>
							<sl-icon name="translate" label={m.UnmarkNoTranslate()}></sl-icon>
						</sl-button>
					</sl-tooltip>
				{/if}
			</div>
		{/each}
	</div>
{/await}
