<script lang="ts">
	import { compare, diff } from "semver"
	import { marked } from "marked"
	import { sanitise } from "$lib/utils"
	import { commands } from "$lib/bindings"
	import { exists, mkdir, readDir, readTextFile, remove } from "@tauri-apps/plugin-fs"
	import { join } from "@tauri-apps/api/path"
	import { type } from "@tauri-apps/plugin-os"
	import { download } from "@tauri-apps/plugin-upload"
	import { relaunch } from "@tauri-apps/plugin-process"
	import { config, gameInstalls, gui } from "$lib/config.svelte"
	import { open } from "@tauri-apps/plugin-shell"
	import modUpdateData, { type ModUpdate } from "$lib/mod-update-data.svelte"
	import * as m from "$lib/paraglide/messages"
	import Masonry from "svelte-bricks"
	import { versionCache } from "$lib/ephemeral.svelte"
	import { getH3GamePath, getV2ModInfo } from "$lib/mods.svelte"
	import isEqual from "lodash.isequal"

	const smfVersion = commands.rsGetFrameworkVersion()
	let latestSmfVersion: Promise<string> | null = $state(null)

	let developerModeDialog: any = $state()

	async function updateFramework() {
		try {
			frameworkDownloadDialog.show()
			await download(
				`https://github.com/hitman-rs/simple-mod-framework/releases/latest/download/${type()[0].toUpperCase() + type().slice(1)}.zip`,
				"tempArchive",
				({ progressTotal, total }) => (frameworkDownloadProgress = (progressTotal / total) * 100)
			)
			frameworkDownloadDialog.hide()

			await mkdir("update")

			frameworkExtractDialog.show()
			await commands.rsExtractArchive("tempArchive", "update")

			await commands.rsUpdateFramework()

			await relaunch()
		} catch (e) {
			frameworkDownloadDialog.hide()
			frameworkExtractDialog.hide()

			frameworkUpdateError = String(e)
			frameworkUpdateErrorDialog.show()
		}
	}

	async function updateMod(update: ModUpdate) {
		try {
			if (update.type === "autoUpdateAvailable") {
				modDownloadDialog.show()
				await download(update.downloadURL, "tempArchive", ({ progressTotal, total }) => (modDownloadProgress = (progressTotal / total) * 100))
				modDownloadDialog.hide()

				if (await exists("extraction")) {
					await remove("extraction", { recursive: true })
				}
				if (await exists("extraction-2")) {
					await remove("extraction-2", { recursive: true })
				}

				modExtractDialog.show()
				await commands.rsExtractArchive("tempArchive", "extraction")
				await remove("tempArchive")

				const files = await readDir("extraction")
				if (files.length === 1 && files[0].isDirectory) {
					// Legacy framework mod
					await mkdir("extraction-2")
					await commands.rsCopyFolder(await join("extraction", files[0].name), "extraction-2")
					await remove("extraction", { recursive: true })

					try {
						const modsInfo = await getV2ModInfo()

						await commands.rsUpgradeMod(await getH3GamePath(config), modsInfo, "extraction-2")
					} catch (e) {
						await remove("extraction-2", { recursive: true })

						modExtractDialog.hide()

						validationMessage = String(e)
						validationDialog.show()
						return
					}

					const validation = await commands.rsValidateModFolder("extraction-2", true)

					if (validation.result === "pass") {
						const oldManifest = JSON.parse(await readTextFile(await join("Mods", update.modId, "manifest.json")))
						const newManifest = JSON.parse(await readTextFile(await join("extraction-2", "manifest.json")))

						const getPeacockPlugins = (manifest: any) => [...(manifest.data?.peacockPlugins || []), ...(manifest.options || []).flatMap((opt) => getPeacockPlugins(opt))]

						const oldPlugins = getPeacockPlugins(oldManifest)
						const newPlugins = getPeacockPlugins(newManifest)

						let anyPluginChanged = false

						for (const plugin of newPlugins) {
							if (!oldPlugins.includes(plugin)) {
								anyPluginChanged = true
								break
							} else {
								const oldPluginContent = await readTextFile(await join("Mods", update.modId, plugin))
								const newPluginContent = await readTextFile(await join("extraction-2", plugin))

								if (oldPluginContent !== newPluginContent) {
									anyPluginChanged = true
									break
								}
							}
						}

						finalisingMod = update.modId

						if (anyPluginChanged) {
							modExtractDialog.hide()
							peacockPluginWarningDialog.show()
							peacockPluginContinueDisabled = true
							setTimeout(() => {
								peacockPluginContinueDisabled = false
							}, 10000)
						} else {
							await checkSDKMods(update.modId)
						}
					} else {
						await remove("extraction-2", { recursive: true })

						modExtractDialog.hide()

						validationMessage = `Mod failed validation after automatic upgrade:\n${validation.message}`
						validationDialog.show()
					}
				} else {
					// Modern framework mod
					const validation = await commands.rsValidateModFolder("extraction", false)

					if (validation.result === "pass") {
						const oldManifest = JSON.parse(await readTextFile(await join("Mods", update.modId, "manifest.json")))
						const newManifest = JSON.parse(await readTextFile(await join("extraction", "manifest.json")))

						const getPeacockPlugins = (manifest: any) => [...(manifest.data?.peacockPlugins || []), ...(manifest.options || []).flatMap((opt) => getPeacockPlugins(opt))]

						const oldPlugins = getPeacockPlugins(oldManifest)
						const newPlugins = getPeacockPlugins(newManifest)

						let anyPluginChanged = false

						for (const plugin of newPlugins) {
							if (!oldPlugins.includes(plugin)) {
								anyPluginChanged = true
								break
							} else {
								const oldPluginContent = await readTextFile(await join("Mods", update.modId, plugin))
								const newPluginContent = await readTextFile(await join("extraction", plugin))

								if (oldPluginContent !== newPluginContent) {
									anyPluginChanged = true
									break
								}
							}
						}

						finalisingMod = update.modId

						if (anyPluginChanged) {
							modExtractDialog.hide()
							peacockPluginWarningDialog.show()
							peacockPluginContinueDisabled = true
							setTimeout(() => {
								peacockPluginContinueDisabled = false
							}, 10000)
						} else {
							await checkSDKMods(update.modId)
						}
					} else {
						await remove("extraction", { recursive: true })

						modExtractDialog.hide()

						validationMessage = validation.message
						validationDialog.show()
					}
				}
			} else {
				throw new Error("Can't automatically update this mod!")
			}
		} catch (e) {
			modDownloadDialog.hide()
			modExtractDialog.hide()

			modUpdateError = String(e)
			modUpdateErrorDialog.show()
		}
	}

	async function checkSDKMods(modId: string) {
		const newManifest = JSON.parse(await readTextFile(await join("extraction", "manifest.json")))

		if (JSON.stringify(newManifest).includes("sdkMods")) {
			modExtractDialog.hide()
			sdkModWarningDialog.show()
			sdkModContinueDisabled = true
			setTimeout(() => {
				sdkModContinueDisabled = false
			}, 10000)
		} else {
			modExtractDialog.show()
			await finaliseModUpdate(modId)
		}
	}

	async function finaliseModUpdate(modId: string) {
		await remove(await join("Mods", modId), { recursive: true })

		// Just in case the mod ID has changed
		const newManifest = JSON.parse(await readTextFile(await join("extraction", "manifest.json")))
		const newId = newManifest.id

		if (modId !== newId) {
			const deployOrderIndex = config.deployOrder.indexOf(modId)
			if (deployOrderIndex !== -1) {
				config.deployOrder[deployOrderIndex] = newId
			}

			if (config.modOptions[modId]) {
				config.modOptions[newId] = config.modOptions[modId]
				delete config.modOptions[modId]
			}

			for (const profile of Object.values(config.profiles)) {
				const deployOrderIndex = profile.deployOrder.indexOf(modId)
				if (deployOrderIndex !== -1) {
					profile.deployOrder[deployOrderIndex] = newId
				}

				if (profile.modOptions[modId]) {
					profile.modOptions[newId] = profile.modOptions[modId]
					delete profile.modOptions[modId]
				}
			}
		}

		await mkdir(await join("Mods", newManifest.id))
		await commands.rsCopyFolder("extraction", await join("Mods", newManifest.id))
		await remove("extraction", { recursive: true })
		gui(config).knownMods = [...gui(config).knownMods, newManifest.id]

		modUpdateData.updates.find((a) => a.type === "autoUpdateAvailable" && a.modId === modId)!.type = "upToDate"
	}

	let sdkUpdate: { type: "sdkUpdate"; data: { oldVersion: string; newVersion: string } } | null = $state(null)

	let frameworkUpdateErrorDialog: any = $state()
	let frameworkUpdateError = $state("")

	let frameworkDownloadDialog: any = $state()
	let frameworkDownloadProgress = $state(0)

	let frameworkExtractDialog: any = $state()

	let modUpdateErrorDialog: any = $state()
	let modUpdateError = $state("")

	let modDownloadDialog: any = $state()
	let modDownloadProgress = $state(0)

	let modExtractDialog: any = $state()

	let validationDialog: any = $state()
	let validationMessage = $state("")

	let finalisingMod = $state("")
	let peacockPluginWarningDialog: any = $state()
	let peacockPluginContinueDisabled = $state(false)
	let sdkModWarningDialog: any = $state()
	let sdkModContinueDisabled = $state(false)

	let doneInitialCheck = $state(false)
	$effect(() => {
		if (!doneInitialCheck && !config["PLACEHOLDER"] && versionCache.time) {
			doneInitialCheck = true
			;(async () => {
				if (gui(config).firstTimeUser) {
					// Automatically select latest game version installed
					config.gamePath =
						(await gameInstalls).toSorted((a, b) => {
							return b.version.localeCompare(a.version)
						})[0]?.path || null

					developerModeDialog.show()
				}

				latestSmfVersion = versionCache.versions.smf ? Promise.resolve(versionCache.versions.smf) : commands.rsGetLatestSmfVersion()
				versionCache.versions.smf = (await latestSmfVersion)!

				// Check SDK updates
				if (!config.gamePath) return

				const gameInstall = await commands.rsDetectGame(config.gamePath)
				if (!gameInstall) return

				const installed = await commands.rsGetInstalledSdkVersion(config.gamePath)
				if (!installed) return

				const isArtifact = !installed.includes(".")
				const latest = versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`]
					? versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`]
					: await (isArtifact ? commands.rsGetLatestSdkArtifact(gameInstall.version) : commands.rsGetLatestSdkRelease(gameInstall.version)).then((ver) => {
							versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`] = ver
							return ver
						})

				if (isArtifact ? installed !== latest : compare(installed, latest) < 0) {
					sdkUpdate = {
						type: "sdkUpdate",
						data: {
							oldVersion: installed,
							newVersion: latest
						}
					}
				}
			})()
		}
	})
</script>

<sl-dialog label={m.PeacockPluginUpdateWarning()} bind:this={peacockPluginWarningDialog}>
	<p>{m.PeacockPluginUpdateWarningDesc()}</p>
	<div slot="footer">
		<sl-button
			variant="danger"
			disabled={peacockPluginContinueDisabled}
			onclick={async () => {
				peacockPluginWarningDialog.hide()
				await checkSDKMods(finalisingMod)
			}}>{peacockPluginContinueDisabled ? m.ButtonTemporarilyDisabled() : m.ContinueButton()}</sl-button
		>
		<sl-button
			variant="default"
			onclick={async () => {
				peacockPluginWarningDialog.hide()
				await remove("extraction", { recursive: true })
			}}>{m.CancelButton()}</sl-button
		>
	</div>
</sl-dialog>

<sl-dialog label={m.SDKModWarning()} bind:this={sdkModWarningDialog}>
	<p>{m.SDKModWarningDesc()}</p>
	<div slot="footer">
		<sl-button
			variant="danger"
			disabled={sdkModContinueDisabled}
			onclick={async () => {
				sdkModWarningDialog.hide()
				modExtractDialog.show()
				await finaliseModUpdate(finalisingMod)
			}}>{sdkModContinueDisabled ? m.ButtonTemporarilyDisabled() : m.ContinueButton()}</sl-button
		>
		<sl-button
			variant="default"
			onclick={async () => {
				sdkModWarningDialog.hide()
				await remove("extraction", { recursive: true })
			}}>{m.CancelButton()}</sl-button
		>
	</div>
</sl-dialog>

<sl-dialog label={m.ErrorUpdatingMod()} bind:this={modUpdateErrorDialog}>
	{m.ErrorUpdatingModDesc()}
	<pre><code>{modUpdateError}</code></pre>

	<sl-button slot="footer" variant="primary" onclick={modUpdateErrorDialog.hide}>
		{m.OKButton()}
	</sl-button>
</sl-dialog>

<sl-dialog label={m.UpdatingMod()} bind:this={modDownloadDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	<div class="mb-2">{m.ModDownloadInProgress()}</div>
	<sl-progress-bar value={modDownloadProgress}></sl-progress-bar>
</sl-dialog>

<sl-dialog label={m.UpdatingMod()} bind:this={modExtractDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	{m.ModExtractionInProgress()}
</sl-dialog>

<sl-dialog label={m.ErrorUpdatingSMF()} bind:this={frameworkUpdateErrorDialog}>
	{m.ErrorUpdatingSMFDesc()}
	<pre><code>{frameworkUpdateError}</code></pre>

	<sl-button slot="footer" variant="primary" onclick={frameworkUpdateErrorDialog.hide}>
		{m.OKButton()}
	</sl-button>
</sl-dialog>

<sl-dialog label={m.UpdatingFramework()} bind:this={frameworkDownloadDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	<div class="mb-2">{m.SMFDownloadInProgress()}</div>
	<sl-progress-bar value={frameworkDownloadProgress}></sl-progress-bar>
</sl-dialog>

<sl-dialog label={m.UpdatingFramework()} bind:this={frameworkExtractDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	{m.SMFExtractionInProgress()}
</sl-dialog>

<sl-dialog label={m.CouldntUpdateMod()} bind:this={validationDialog}>
	{m.CouldntUpdateModDesc()}
	<pre><code>{validationMessage}</code></pre>
</sl-dialog>

<sl-dialog label={m.DeveloperModeUpsell()} bind:this={developerModeDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	{m.DeveloperModeUpsellDesc()}

	<div class="flex gap-2" slot="footer">
		<sl-button
			variant="primary"
			onclick={() => {
				config.developerMode = false
				gui(config).firstTimeUser = false
				developerModeDialog.hide()
			}}>{m.AmModUser()}</sl-button
		>
		<sl-button
			variant="primary"
			onclick={() => {
				config.developerMode = true
				gui(config).firstTimeUser = false
				developerModeDialog.hide()
			}}>{m.AmModDev()}</sl-button
		>
	</div>
</sl-dialog>

<h1 class="text-4xl 2xl:text-5xl font-bold mb-2 2xl:mb-4">{m.WelcomeToSMF()} <span class="bg-clip-text bg-gradient-to-r text-transparent from-primary-300 to-teal-300">Simple Mod Framework</span>!</h1>
<div class="text-[0.95rem] 2xl:text-base h-[calc(100vh-12rem)] overflow-y-auto">
	{#await commands.rsGetExperiment() then experiment}
		<Masonry
			style="justify-content: start"
			gap={15}
			minColWidth={screen.availWidth / 6}
			order="row-first"
			items={[
				...(!config.onlineServices ? [{ type: "onlineServices" } as const] : []),
				...(experiment ? [{ type: "experiment" } as const] : []),
				{ type: "smfVersion" } as const,
				...(sdkUpdate ? [sdkUpdate] : []),
				...(modUpdateData.checkingFinished && modUpdateData.updates.every((a) => a.type === "upToDate")
					? [{ type: "modsUpToDate" } as const]
					: modUpdateData.updates
							.filter((a, idx, arr) => arr.findIndex((b) => isEqual(a, b)) === idx)
							.filter((a) => a.type !== "upToDate")
							.sort((a, b) => {
								const order = ["autoUpdateAvailable", "nexusUpdateAvailable", "developmentVersion", "failed"]
								return order.indexOf(a.type) - order.indexOf(b.type)
							})
							.map((a) => ({ type: "modUpdate", data: a }) as const)),
				...(modUpdateData.skipped > 0 ? [{ type: "updateCheckSkipped", skipped: modUpdateData.skipped } as const] : []),
				...(!modUpdateData.checkingFinished ? [{ type: "updateChecking" } as const] : [])
			]}
			getId={(item) => (item.type === "modUpdate" ? `${item.data.type}-${item.data.modName}` : item.type)}
		>
			{#snippet children({ item })}
				{#if item.type === "onlineServices"}
					<sl-card class="w-full">
						<div slot="header">
							<h2 class="text-xl 2xl:text-2xl font-bold">{m.OnlineServicesDisabled()}</h2>
						</div>
						<div>
							{m.OnlineServicesDisabledDesc()}
						</div>
						<div slot="footer"
							><sl-button
								variant="primary"
								onclick={() => {
									config.onlineServices = true
								}}>{m.EnableButton()}</sl-button
							></div
						>
					</sl-card>
				{:else if item.type === "experiment"}
					<sl-card class="w-full">
						<div slot="header">
							<h2 class="text-xl 2xl:text-2xl font-bold">{m.ExperimentalVersion()}</h2>
						</div>
						<div>{m.ExperimentalVersionDesc()} (<code>{experiment}</code>).</div>
					</sl-card>
				{:else if item.type === "sdkUpdate"}
					<sl-card class="w-full">
						<div slot="header">
							<h2 class="text-xl 2xl:text-2xl font-bold">{m.ZHMModSDKUpdateCard()}</h2>
						</div>
						<div>{m.ZHMModSDKUpdateCardDesc(item.data)}</div>
						<div slot="footer"><sl-button variant="primary" href="/settings">{m.GoToSettingsButton()}</sl-button></div>
					</sl-card>
				{:else if item.type === "smfVersion"}
					{#await smfVersion then smfVersion}
						{#if latestSmfVersion}
							{#await latestSmfVersion}
								<sl-card class="w-full">
									{m.CheckingForUpdates()}
								</sl-card>
							{:then latestSmfVersion}
								{#if compare(smfVersion, latestSmfVersion) < 0}
									{#await (async () => versionCache.versions["smf#changelog"] || (versionCache.versions["smf#changelog"] = await commands.rsGetSmfChangelog()))() then smfChangelog}
										<sl-card class="w-full">
											<div slot="header">
												<h2 class="text-xl 2xl:text-2xl font-bold"
													>{diff(smfVersion, latestSmfVersion) == "major"
														? m.MajorUpdateAvailable()
														: diff(smfVersion, latestSmfVersion) == "minor"
															? m.MinorUpdateAvailable()
															: m.PatchUpdateAvailable()}</h2
												>
												<p class="mt-0.5">{m.UpdateFromTo({ from: smfVersion, to: latestSmfVersion })}</p>
											</div>
											<div class="changelog">
												{#await marked(smfChangelog, { gfm: true }) then x}{@html sanitise(x)}{/await}
											</div>
											<div slot="footer"><sl-button variant="primary" onclick={updateFramework}>{m.UpdateButton()}</sl-button></div>
										</sl-card>
									{:catch}
										<sl-card class="w-full">{m.CouldntCheckFrameworkUpdates()}</sl-card>
									{/await}
								{:else}
									<sl-card class="w-full">
										<div slot="header">
											<h2 class="text-xl 2xl:text-2xl font-bold">{m.SMFUpToDate()}</h2>
										</div>
										<div>{m.SMFUpToDateDesc({ ver: smfVersion })}</div>
									</sl-card>
								{/if}
							{:catch}
								<sl-card class="w-full">{m.CouldntCheckFrameworkUpdates()}</sl-card>
							{/await}
						{/if}
					{/await}
				{:else if item.type === "updateChecking"}
					<sl-card class="w-full">{m.CheckingModUpdate({ mod: modUpdateData.modBeingChecked })}</sl-card>
				{:else if item.type === "updateCheckSkipped"}
					<sl-card class="w-full">
						<div slot="header">
							<h2 class="text-xl 2xl:text-2xl font-bold">{m.ModUpdateRateLimit()}</h2>
						</div>
						<div>{m.ModUpdateRateLimitDesc({ skipped: modUpdateData.skipped })}</div>
					</sl-card>
				{:else if item.type === "modsUpToDate"}
					<sl-card class="w-full">
						<div slot="header">
							<h2 class="text-xl 2xl:text-2xl font-bold">{m.ModsUpToDate()}</h2>
						</div>
						<div>{m.ModsUpToDateDesc()}</div>
					</sl-card>
				{:else if item.type === "modUpdate"}
					{@const update = item.data}
					{#if update.type === "failed"}
						<sl-card class="w-full">
							{m.CouldntCheckModUpdate({ mod: update.modName })}
						</sl-card>
					{:else if update.type === "developmentVersion"}
						<sl-card class="w-full">
							<div slot="header">
								<h2 class="text-xl 2xl:text-2xl font-bold leading-tight">{m.DevVersion()}</h2>
							</div>
							<div>{m.DevVersionDesc({ mod: update.modName, ver: update.version })}</div>
						</sl-card>
					{:else if update.type === "nexusUpdateAvailable"}
						<sl-card class="w-full">
							<div slot="header">
								<h2 class="text-xl 2xl:text-2xl font-bold"
									>{diff(update.oldVersion, update.newVersion) == "major"
										? m.MajorUpdateMod({ mod: update.modName })
										: diff(update.oldVersion, update.newVersion) == "minor"
											? m.MinorUpdateMod({ mod: update.modName })
											: m.PatchUpdateMod({ mod: update.modName })}</h2
								>
								<p class="mt-0.5">{m.UpdateFromTo({ from: update.oldVersion, to: update.newVersion })}</p>
							</div>
							<div class="changelog">{m.RedownloadFromNexus()}</div>
							<div slot="footer"><sl-button variant="primary" href="#" onclick={() => open(update.url)}>{m.GoToNexusButton()}</sl-button></div>
						</sl-card>
					{:else if update.type === "autoUpdateAvailable"}
						<sl-card class="w-full">
							<div slot="header">
								<h2 class="text-xl 2xl:text-2xl font-bold"
									>{diff(update.oldVersion, update.newVersion) == "major"
										? m.MajorUpdateMod({ mod: update.modName })
										: diff(update.oldVersion, update.newVersion) == "minor"
											? m.MinorUpdateMod({ mod: update.modName })
											: m.PatchUpdateMod({ mod: update.modName })}</h2
								>
								<p class="mt-0.5">{m.UpdateFromTo({ from: update.oldVersion, to: update.newVersion })}</p>
							</div>
							<div class="changelog">
								{#if update.changelogIsURL}
									<!-- svelte-ignore a11y_invalid_attribute -->
									<a href="#" onclick={() => open(update.changelog)}>{m.ViewChangelogOnline()}</a>
								{:else}
									{#await marked(update.changelog, { gfm: true }) then x}{@html sanitise(x)}{/await}
								{/if}
							</div>
							<div slot="footer"><sl-button variant="primary" href="#" onclick={() => updateMod(update)}>{m.UpdateButton()}</sl-button></div>
						</sl-card>
					{/if}
				{/if}
			{/snippet}
		</Masonry>
	{/await}
</div>
