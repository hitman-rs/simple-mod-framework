<script lang="ts">
	import "./app.css"

	import "@shoelace-style/shoelace/dist/themes/dark.css"
	import { setBasePath } from "@shoelace-style/shoelace/dist/utilities/base-path.js"

	import "@fontsource-variable/public-sans/wght.css"
	import "@fontsource/pt-serif/400.css"
	import "@fontsource/pt-serif/400-italic.css"
	import "@fontsource/pt-serif/700.css"
	import "@fontsource/pt-serif/700-italic.css"
	import "@fontsource-variable/fira-code/wght.css"

	setBasePath("shoelace-assets")

	import "@shoelace-style/shoelace/dist/components/button/button.js"
	import "@shoelace-style/shoelace/dist/components/divider/divider.js"
	import "@shoelace-style/shoelace/dist/components/card/card.js"
	import "@shoelace-style/shoelace/dist/components/dialog/dialog.js"
	import "@shoelace-style/shoelace/dist/components/checkbox/checkbox.js"
	import "@shoelace-style/shoelace/dist/components/radio/radio.js"
	import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js"
	import "@shoelace-style/shoelace/dist/components/input/input.js"
	import "@shoelace-style/shoelace/dist/components/alert/alert.js"
	import "@shoelace-style/shoelace/dist/components/icon/icon.js"
	import "@shoelace-style/shoelace/dist/components/progress-bar/progress-bar.js"
	import "@shoelace-style/shoelace/dist/components/select/select.js"
	import "@shoelace-style/shoelace/dist/components/option/option.js"
	import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js"
	import "@shoelace-style/shoelace/dist/components/color-picker/color-picker.js"

	import "svelte-medium-image-zoom/dist/styles.css"

	import { onDestroy, onMount } from "svelte"
	import { commands } from "$lib/bindings"
	import { goto } from "$app/navigation"
	import { allMods, getModManifest, loc } from "$lib/mods.svelte"
	import { config, saveConfig } from "$lib/config.svelte"
	import modUpdateData from "$lib/mod-update-data.svelte"
	import { compare } from "semver"
	import { download } from "@tauri-apps/plugin-upload"
	import { fetch } from "@tauri-apps/plugin-http"
	import * as m from "$lib/paraglide/messages"
	import { overwriteGetLocale, locales } from "$lib/paraglide/runtime"
	import { watch } from "runed"
	import { page } from "$app/state"
	import { listen } from "@tauri-apps/api/event"
	import { saveVersionCache, versionCache } from "$lib/ephemeral.svelte"

	overwriteGetLocale(() => config.uiLocale as (typeof locales)[number])

	interface Props {
		children?: import("svelte").Snippet
	}

	let { children }: Props = $props()

	let unlisten = () => {}

	onMount(async () => {
		unlisten = await listen<string>("scheme-request-received", (evt) => {
			if (evt.payload.startsWith("simple-mod-framework://install/")) {
				goto("/download-mod?url=" + encodeURIComponent(evt.payload.replace("simple-mod-framework://install/", "")))
			}
		})
	})

	onDestroy(unlisten)

	let startedGatheringData = $state(false)

	let modUpdatesNotification: any = $state()
	let hashListUpdateNotification: any = $state()
	$effect(() => {
		if (!startedGatheringData && !config["PLACEHOLDER"] && hashListUpdateNotification && versionCache.time) {
			startedGatheringData = true
			;(async () => {
				hashListUpdateNotification.toast()

				try {
					const currentTTHLVersion = (await commands.rsGetTonytoolsHashListVersion()) || 0

					const TTHLRepo = "https://github.com/glacier-modding/Hitman-l10n-Hashes/releases/latest/download"

					const newTTHLVersion =
						versionCache.versions[`${TTHLRepo}/version.json`] ||
						String(
							(
								await (
									await fetch(`${TTHLRepo}/version.json`, {
										method: "GET"
									})
								).json()
							).version
						)
					versionCache.versions[`${TTHLRepo}/version.json`] = newTTHLVersion

					if (currentTTHLVersion < +newTTHLVersion) {
						await download(`${TTHLRepo}/hash_list.hmla`, "tonytools_hash_list.hmla")
					}
				} catch (e) {
					console.log("Failed to update hash lists", e)
				}
				hashListUpdateNotification.hide()

				let nexusRatelimit = false
				let ghRatelimit = false

				for (const mod of allMods) {
					const manifest = await getModManifest(mod)

					modUpdateData.modBeingChecked = loc(manifest.name)

					try {
						if (manifest.url) {
							const url = new URL(manifest.url)

							if (url.hostname === "nexusmods.com" || url.hostname === "www.nexusmods.com") {
								if (nexusRatelimit) {
									modUpdateData.skipped += 1
									continue
								}

								const version =
									versionCache.versions[manifest.url] ||
									(config.onlineServices
										? await commands.rsGetProxiedModVersion(manifest.url)
										: await new Promise((r) => setTimeout(r, 1050)).then(() =>
												commands.rsGetNexusVersion(
													url.pathname
														.split("/")
														.filter((a) => a !== "")
														.at(-3) || "",
													parseInt(
														url.pathname
															.split("/")
															.filter((a) => a !== "")
															.at(-1) || ""
													)
												)
											))

								versionCache.versions[manifest.url] = version

								if (compare(manifest.version, version) < 0) {
									modUpdateData.updates.push({ type: "nexusUpdateAvailable", oldVersion: manifest.version, newVersion: version, modName: loc(manifest.name), url: manifest.url })
								} else if (compare(manifest.version, version) > 0) {
									modUpdateData.updates.push({ type: "developmentVersion", version: manifest.version, modName: loc(manifest.name) })
								} else {
									modUpdateData.updates.push({ type: "upToDate", modName: loc(manifest.name) })
								}
							} else if (url.hostname === "github.com") {
								if (ghRatelimit) {
									modUpdateData.skipped += 1
									continue
								}

								const repository = url.pathname
									.split("/")
									.filter((a) => a !== "")
									.slice(-2)
									.join("/")

								const version =
									versionCache.versions[manifest.url] ||
									(config.onlineServices
										? await commands.rsGetProxiedModVersion(manifest.url)
										: await new Promise((r) => setTimeout(r, 1050)).then(() => commands.rsGetGithubVersion(repository)))

								versionCache.versions[manifest.url] = version

								if (compare(manifest.version, version) < 0) {
									modUpdateData.updates.push({
										type: "autoUpdateAvailable",
										oldVersion: manifest.version,
										newVersion: version,
										modName: loc(manifest.name),
										modId: manifest.id,
										changelogIsURL: false,
										downloadURL:
											versionCache.versions[`${manifest.url}#download`] ||
											(versionCache.versions[`${manifest.url}#download`] = await new Promise((r) => setTimeout(r, 1050)).then(() => commands.rsGetGithubDownloadUrl(repository))),
										changelog:
											versionCache.versions[`${manifest.url}#changelog`] ||
											(versionCache.versions[`${manifest.url}#changelog`] = await new Promise((r) => setTimeout(r, 1050)).then(() =>
												commands.rsGetGithubChangelog(manifest.version, repository)
											))
									})
								} else if (compare(manifest.version, version) > 0) {
									modUpdateData.updates.push({ type: "developmentVersion", version: manifest.version, modName: loc(manifest.name) })
								} else {
									modUpdateData.updates.push({ type: "upToDate", modName: loc(manifest.name) })
								}
							} else if (url.hostname === "modworkshop.net") {
								const id = parseInt(
									url.pathname
										.split("/")
										.filter((a) => a !== "")
										.at(-1) || ""
								)

								const version =
									versionCache.versions[manifest.url] || (config.onlineServices ? await commands.rsGetProxiedModVersion(manifest.url) : await commands.rsGetModworkshopVersion(id))

								versionCache.versions[manifest.url] = version

								if (compare(manifest.version, version) < 0) {
									modUpdateData.updates.push({
										type: "autoUpdateAvailable",
										oldVersion: manifest.version,
										newVersion: version,
										modName: loc(manifest.name),
										modId: manifest.id,
										changelogIsURL: true,
										downloadURL:
											versionCache.versions[`${manifest.url}#download`] || (versionCache.versions[`${manifest.url}#download`] = await commands.rsGetModworkshopDownloadUrl(id)),
										changelog: `https://modworkshop.net/mod/${id}?tab=changelog`
									})
								} else if (compare(manifest.version, version) > 0) {
									modUpdateData.updates.push({ type: "developmentVersion", version: manifest.version, modName: loc(manifest.name) })
								} else {
									modUpdateData.updates.push({ type: "upToDate", modName: loc(manifest.name) })
								}
							}
						}
					} catch (e) {
						console.error("Failed to check for updates for", manifest.id, e)

						if (String(e).includes("missing field `assets`")) {
							ghRatelimit = true
							modUpdateData.skipped += 1
						} else if (String(e).includes("<title>Just a moment...</title>")) {
							nexusRatelimit = true
							modUpdateData.skipped += 1
						} else {
							modUpdateData.updates.push({ type: "failed", modName: loc(manifest.name), error: String(e) })
						}
					}
				}

				modUpdateData.checkingFinished = true
			})()
		}
	})

	$effect(() => {
		if (modUpdatesNotification) {
			if (page.url.pathname !== "/" && !modUpdateData.checkingFinished && !modUpdatesNotification.open) {
				modUpdatesNotification.toast()
			} else if ((page.url.pathname === "/" || modUpdateData.checkingFinished) && modUpdatesNotification.open) {
				modUpdatesNotification.hide()
			}
		}
	})

	watch(
		() => $state.snapshot(config),
		() => void saveConfig()
	)

	watch(
		() => $state.snapshot(versionCache),
		() => void saveVersionCache()
	)
</script>

{#if page.url.pathname !== "/deploy"}
	<div class="px-16 pt-8 mb-2 2xl:mb-4 flex flex-wrap gap-4 text-lg 2xl:text-xl underline-offset-4 decoration-2">
		<!-- Decoration colour has to be duplicated because Webkit on Linux won't allow it to be on the parent -->
		<a href="/" class="decoration-primary-400" class:underline={page.url.pathname === "/"}>{m.HomePage()}</a>
		<a href="/mods" class="decoration-primary-400" class:underline={page.url.pathname === "/mods" || page.url.pathname === "/mod-options"}>{m.ModsPage()}</a>
		<a href="/profiles" class="decoration-primary-400" class:underline={page.url.pathname === "/profiles"}>{m.Profiles()}</a>
		<a href="/settings" class="decoration-primary-400" class:underline={page.url.pathname === "/settings"}>{m.SettingsPage()}</a>
		<a href="/credits" class="decoration-primary-400" class:underline={page.url.pathname === "/credits"}>{m.CreditsPage()}</a>
	</div>
{/if}

<div class="px-16 w-full">
	{@render children?.()}
</div>

<sl-alert variant="primary" bind:this={modUpdatesNotification}>
	<h4 class="text-xl font-bold">{m.CheckingModUpdates()}</h4>
	{m.CheckingModUpdate({ mod: modUpdateData.modBeingChecked })}
</sl-alert>

<sl-alert variant="primary" bind:this={hashListUpdateNotification}>
	<h4 class="text-xl font-bold">{m.CheckingHashListUpdates()}</h4>
	{m.CheckingHashListUpdatesDesc()}
</sl-alert>

<style>
	:global(#sveltekit-body) {
		font-family: "Public Sans Variable", sans-serif;
		--sl-font-sans: "Public Sans Variable", sans-serif;
		--sl-font-serif: "PT Serif", serif;
		--sl-font-mono: "Fira Code Variable", monospace;

		--sl-color-primary-950: rgb(243 251 255);
		--sl-color-primary-900: rgb(214 240 255);
		--sl-color-primary-800: rgb(181 229 255);
		--sl-color-primary-700: rgb(143 216 255);
		--sl-color-primary-600: rgb(89 196 255);
		--sl-color-primary-500: rgb(6 167 255);
		--sl-color-primary-400: rgb(0 138 213);
		--sl-color-primary-300: rgb(0 113 175);
		--sl-color-primary-200: rgb(0 94 145);
		--sl-color-primary-100: rgb(0 67 103);
		--sl-color-primary-50: rgb(0 42 64);

		background-color: var(--sl-color-neutral-0);
		color: white;
		color-scheme: dark;
	}

	:global(.changelog h1) {
		font-size: 1.5rem;
		font-weight: 700;
		@apply mb-2;
	}

	:global(.changelog h2) {
		font-size: 1.25rem;
		font-weight: 700;
		@apply mb-1;
	}

	:global(.changelog h3) {
		font-weight: 700;
	}

	:global(.changelog li) {
		list-style-position: inside;
		list-style-type: disclosure-closed;
	}

	:global(code) {
		font-family: "Fira Code Variable", monospace;
	}

	:global(sl-dialog.noClose::part(close-button)) {
		display: none;
	}

	:global(.sl-toast-stack) {
		top: auto;
		bottom: 2rem;
	}

	:global([data-smiz-modal-overlay="visible"]) {
		@apply bg-black/80;
	}
</style>
