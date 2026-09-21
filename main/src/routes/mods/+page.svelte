<script lang="ts">
	import { config, gameInstalls, gui } from "$lib/config.svelte"
	import { chunkPartitions, getModManifest, reformatModID, getH3GamePath, loc, getV2ModInfo, allMods, getAllOptions } from "$lib/mods.svelte"
	import { SortableList } from "@jhubbardsf/svelte-sortablejs"
	import { mkdir, exists, readDir, remove, writeTextFile, readTextFile } from "@tauri-apps/plugin-fs"
	import { join, localDataDir } from "@tauri-apps/api/path"
	import { open } from "@tauri-apps/plugin-dialog"
	import type { DeploySummary, Manifest } from "$lib/bindings"
	import { commands, events, type Config } from "$lib/bindings"
	import Ajv from "ajv/dist/2020"
	import { listen, type UnlistenFn } from "@tauri-apps/api/event"
	import { fade } from "svelte/transition"
	import * as m from "$lib/paraglide/messages"
	import { onDestroy } from "svelte"
	import md5 from "md5"
	import isEqual from "lodash.isequal"
	import { watch } from "runed"
	import { page } from "$app/state"
	import { Masonry } from "svelte-bricks"

	const validateManifest = (async () => new Ajv({ validateFormats: false }).compile<Manifest>((await commands.rsGetManifestSchema()) as unknown as any))()

	let rpkgModNameDialog: any = $state()
	let rpkgModName = $state("")
	let rpkgPath = ""

	let validationDialog: any = $state()
	let validationMessage = $state("")

	let upgradeErrorDialog: any = $state()
	let upgradeErrorMessage = $state("")

	let addingModDialog: any = $state()
	let modUpgrading = $state(false)
	let addingModProgressText = $state("")

	let rpkgConfirmDialog: any = $state()

	let batchModAddingDialog: any = $state()
	let batchModBeingAdded = $state("A mod")
	let failedModsDialog: any = $state()
	let failedMods: [string, string][] = $state([])

	async function addMod(file: string) {
		modUpgrading = false
		addingModDialog.show()

		if (file.endsWith(".rpkg")) {
			rpkgPath = file
			addingModDialog.hide()
			rpkgConfirmDialog.show()
		} else {
			if (await exists("extraction")) {
				await remove("extraction", { recursive: true })
			}
			if (await exists("extraction-2")) {
				await remove("extraction-2", { recursive: true })
			}

			await commands.rsExtractArchive(file, "extraction")

			const rpkg = await findRPKG("extraction")

			if (rpkg) {
				rpkgPath = rpkg
				addingModDialog.hide()
				rpkgConfirmDialog.show()
			} else {
				const files = await readDir("extraction")
				if (files.length === 1 && files[0].isDirectory) {
					// Legacy framework mod
					await mkdir("extraction-2")
					await commands.rsCopyFolder(await join("extraction", files[0].name), "extraction-2")
					await remove("extraction", { recursive: true })

					try {
						modUpgrading = true

						const modsInfo = await getV2ModInfo()

						await commands.rsUpgradeMod(await getH3GamePath(config), modsInfo, "extraction-2")
					} catch (e) {
						await remove("extraction-2", { recursive: true })
						upgradeErrorMessage = String(e)
						addingModDialog.hide()
						upgradeErrorDialog.show()
						return
					}

					const validation = await commands.rsValidateModFolder("extraction-2", true)

					if (validation.result === "pass") {
						const manifest = JSON.parse(await readTextFile(await join("extraction-2", "manifest.json")))

						if (JSON.stringify(manifest).includes("peacockPlugins")) {
							addingModDialog.hide()
							finalisingMod = "extraction-2"
							peacockPluginWarningDialog.show()
							peacockPluginContinueDisabled = true
							setTimeout(() => {
								peacockPluginContinueDisabled = false
							}, 10000)
						} else {
							await checkSDKMods("extraction-2")
						}
					} else {
						addingModDialog.hide()
						await remove("extraction-2", { recursive: true })
						validationMessage = `Mod failed validation after automatic upgrade:\n${validation.message}`
						validationDialog.show()
					}
				} else {
					// Modern framework mod
					const validation = await commands.rsValidateModFolder("extraction", false)

					if (validation.result === "pass") {
						const manifest = JSON.parse(await readTextFile(await join("extraction", "manifest.json")))

						if (JSON.stringify(manifest).includes("peacockPlugins")) {
							addingModDialog.hide()
							finalisingMod = "extraction"
							peacockPluginWarningDialog.show()
							peacockPluginContinueDisabled = true
							setTimeout(() => {
								peacockPluginContinueDisabled = false
							}, 10000)
						} else {
							await checkSDKMods("extraction")
						}
					} else {
						addingModDialog.hide()
						await remove("extraction", { recursive: true })
						validationMessage = validation.message
						validationDialog.show()
					}
				}
			}
		}
	}

	async function checkSDKMods(modFolder: string) {
		const manifest = JSON.parse(await readTextFile(await join(modFolder, "manifest.json")))

		if (JSON.stringify(manifest).includes("sdkMods")) {
			addingModDialog.hide()
			finalisingMod = modFolder
			sdkModWarningDialog.show()
			sdkModContinueDisabled = true
			setTimeout(() => {
				sdkModContinueDisabled = false
			}, 10000)
		} else {
			addingModDialog.show()
			await finaliseModInstall(modFolder)
		}
	}

	async function finaliseModInstall(modFolder: string) {
		addingModProgressText = "Copying mod files"
		const manifest = JSON.parse(await readTextFile(await join(modFolder, "manifest.json")))

		if (await exists(await join("Mods", manifest.id))) {
			await remove(await join("Mods", manifest.id), { recursive: true })
		}

		await commands.rsCopyFolder(modFolder, await join("Mods", manifest.id))
		gui(config).knownMods = [...gui(config).knownMods, manifest.id]

		await remove(modFolder, { recursive: true })

		addingModDialog.hide()
	}

	async function continueRPKGModInstallation() {
		addingModDialog.show()

		const id = reformatModID(
			`RPKGMod.${rpkgModName
				.split("")
				.filter((a) => /[a-zA-Z]/.test(a))
				.join("")
				.replaceAll(" ", "-")}`
		)

		await mkdir(await join("Mods", id))

		await writeTextFile(
			await join("Mods", id, "manifest.json"),
			JSON.stringify({
				id,
				name: rpkgModName,
				authors: ["Unknown"],
				description: "An RPKG mod converted to work with the Simple Mod Framework.",
				url: null,
				version: "1.0.0",
				frameworkVersion: "3.0.0",
				conditions: {
					supportedGames: ["h3"]
				},
				data: {
					contentFolders: ["content"]
				}
			} satisfies Manifest)
		)

		await mkdir(await join("Mods", id, "content"))

		let chunk = "chunk0"

		let result = [...rpkgPath.matchAll(/(chunk[0-9]+)(?:patch.*)?/g)]
		if (result.length) {
			chunk = result[0][1]
		}

		if (!Object.hasOwn(chunkPartitions, chunk)) {
			addingModDialog.hide()
			validationMessage = `The RPKG file is for an unrecognised chunk and can't be automatically converted.`
			validationDialog.show()
			return
		}

		await commands.rsExtractRpkg(rpkgPath, await join("Mods", id, "content", chunkPartitions[chunk as keyof typeof chunkPartitions]))

		gui(config).knownMods = [...gui(config).knownMods, id]

		addingModDialog.hide()
	}

	async function findRPKG(path: string): Promise<string | null> {
		const files = await readDir(path)
		for (const file of files) {
			if (file.name && file.name.endsWith(".rpkg")) {
				return await join(path, file.name)
			}

			if (file.isDirectory) {
				const found = await findRPKG(await join(path, file.name))
				if (found) {
					return found
				}
			}
		}

		return null
	}

	const unlisteners: UnlistenFn[] = []

	onDestroy(() => {
		for (const unlisten of unlisteners) {
			unlisten()
		}
	})

	let incorrectlyInstalledModDialog: any = $state()

	let doneInitialCheck = $state(false)
	$effect(() => {
		if (!doneInitialCheck && !config["PLACEHOLDER"]) {
			doneInitialCheck = true
			;(async () => {
				unlisteners.push(
					await listen("tauri://drag-drop", (event) => {
						showDropHint = false

						addMod((event.payload as { paths: string[] }).paths[0])
					})
				)

				unlisteners.push(
					await listen("tauri://drag-enter", () => {
						showDropHint = true
					})
				)

				unlisteners.push(
					await listen("tauri://drag-leave", () => {
						showDropHint = false
					})
				)

				unlisteners.push(
					await events.modUpgradeProgress.listen(({ payload }) => {
						addingModProgressText = payload
					})
				)

				for (const mod of allMods) {
					if (!gui(config).knownMods.includes(mod)) {
						gui(config).knownMods = [...gui(config).knownMods, mod]
						if (!config.developerMode) {
							incorrectlyInstalledModDialog.show()
						}
					}
				}

				const query = page.url.searchParams.get("addAndDeleteFile")
				if (typeof query === "string") {
					addMod(query).then(() => remove(query))
				}
			})()
		}
	})

	let disabledModsFilter = $state("")
	let enabledModsFilter = $state("")

	let showDropHint = $state(false)

	let finalisingMod = $state("")
	let peacockPluginWarningDialog: any = $state()
	let peacockPluginContinueDisabled = $state(false)
	let sdkModWarningDialog: any = $state()
	let sdkModContinueDisabled = $state(false)

	async function needRedeploy(config: Config) {
		if (config.gamePath) {
			const summaryPath = await join(await localDataDir(), "Simple Mod Framework", "deployments", md5(await commands.rsCanonicalizePath(config.gamePath)), "summary.json")

			if (await exists(summaryPath)) {
				const summary = JSON.parse(await readTextFile(summaryPath)) as DeploySummary
				if (!isEqual(config.deployOrder, summary.config.deployOrder)) return true
				if (!isEqual(Object.fromEntries((await Promise.all(config.deployOrder.map((a) => getModManifest(a)))).map((a) => [a.id, a.version])), summary.modVersions)) return true
				if (!isEqual(Object.fromEntries(config.deployOrder.map((a) => [a, config.modOptions[a]])), Object.fromEntries(config.deployOrder.map((a) => [a, summary.config.modOptions[a]]))))
					return true
				if (summary.game.hash !== (await commands.rsDetectGame(config.gamePath).then((a) => a?.hash))) return true
			} else {
				return true
			}
		}

		return false
	}

	let needToRedeploy = $state(false)
	watch(
		() => $state.snapshot(config),
		() => void needRedeploy(config).then((res) => (needToRedeploy = res))
	)
</script>

<sl-dialog label={m.PeacockPluginWarning()} bind:this={peacockPluginWarningDialog}>
	<p>{m.PeacockPluginWarningDesc()}</p>
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
				await remove(finalisingMod, { recursive: true })
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
				addingModDialog.show()
				await finaliseModInstall(finalisingMod)
			}}>{sdkModContinueDisabled ? m.ButtonTemporarilyDisabled() : m.ContinueButton()}</sl-button
		>
		<sl-button
			variant="default"
			onclick={async () => {
				sdkModWarningDialog.hide()
				await remove(finalisingMod, { recursive: true })
			}}>{m.CancelButton()}</sl-button
		>
	</div>
</sl-dialog>

<!-- TODO: Show mod links -->

<sl-dialog label={m.ModName()} bind:this={rpkgModNameDialog}>
	<sl-input label={m.EnterModName()} value={rpkgModName} oninput={(evt) => (rpkgModName = evt.target.value)}></sl-input>
	<br />
	<sl-button
		variant="primary"
		onclick={async () => {
			rpkgModNameDialog.hide()
			await continueRPKGModInstallation()
		}}>{m.ContinueButton()}</sl-button
	>
</sl-dialog>

<sl-dialog label={m.InvalidMod()} bind:this={validationDialog}>
	{m.InvalidModDesc()}
	<pre class="mt-2"><code>{validationMessage}</code></pre>
</sl-dialog>

<sl-dialog label={m.CouldntUpgradeMod()} bind:this={upgradeErrorDialog}>
	{m.CouldntUpgradeModDesc()}
	<pre class="mt-2"><code>{upgradeErrorMessage}</code></pre>
</sl-dialog>

<sl-dialog label={m.AddingMod()} bind:this={addingModDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	<p>{modUpgrading ? m.ModAddUpgradeInProgress() : m.ModAddExtractionInProgress()}</p>
	{#if addingModProgressText}
		<p class="mt-2 text-neutral-300 break-all">{addingModProgressText}</p>
	{/if}
</sl-dialog>

<sl-dialog label={m.ImproperInstalledMod()} bind:this={incorrectlyInstalledModDialog}>
	{m.ImproperInstalledModDesc()}
</sl-dialog>

<sl-dialog label={m.AddingMod()} bind:this={batchModAddingDialog} class="noClose" onsl-request-close={(e) => e.preventDefault()}>
	<p>{batchModBeingAdded} is being upgraded and added to the framework. Please wait.</p>
	{#if addingModProgressText}
		<p class="mt-2 text-neutral-300 break-all">{addingModProgressText}</p>
	{/if}
</sl-dialog>

<!-- FIXME: Remove batch upgrades before release -->
<sl-dialog label="Failed mod upgrades" bind:this={failedModsDialog}>
	<p>The following mods failed to upgrade:</p>
	<div class="mt-4 max-h-[60vh] overflow-y-auto">
		{#each failedMods as [name, error]}
			<div class="mb-4">
				<h3 class="font-bold">{name}</h3>
				<pre class="mt-1 text-sm"><code>{error}</code></pre>
			</div>
		{/each}
	</div>
</sl-dialog>

<sl-dialog label={m.RpkgMod()} bind:this={rpkgConfirmDialog}>
	<p>{m.RpkgModDesc()}</p>
	<div slot="footer">
		<sl-button
			variant="primary"
			onclick={() => {
				rpkgConfirmDialog.hide()
				rpkgModNameDialog.show()
			}}>{m.ContinueButton()}</sl-button
		>
		<sl-button variant="default" onclick={() => rpkgConfirmDialog.hide()}>{m.CancelButton()}</sl-button>
	</div>
</sl-dialog>

<div class="grid grid-cols-2 gap-4">
	<div>
		<h1 class="text-4xl 2xl:text-5xl font-bold mb-2">{m.AvailableMods()}</h1>
		<div class="flex flex-row flex-wrap gap-2">
			<sl-button
				variant="primary"
				onclick={async () => {
					const path = await open({
						title: "Select the mod file",
						filters: [
							{
								name: "Mod files",
								extensions: ["rpkg", "zip", "7z"]
							}
						]
					})

					if (typeof path === "string") {
						addMod(path)
					}
				}}>{m.AddModButton()}</sl-button
			>
			{#if config.developerMode}
				<sl-button
					onclick={async () => {
						const path = await open({
							title: "Select the Mods folder",
							directory: true
						})

						if (typeof path === "string") {
							batchModAddingDialog.show()
							failedMods = []

							const modsInfo = await getV2ModInfo()

							for (const modFolder of (await readDir(path)).filter((a) => a.name !== "Managed by SMF, do not touch")) {
								if (modFolder.name) {
									batchModBeingAdded = modFolder.name
								}

								try {
									if (await exists("working")) {
										await remove("working", { recursive: true })
									}
									await mkdir("working")

									await commands.rsCopyFolder(await join(path, modFolder.name), "working")

									try {
										await commands.rsUpgradeMod(await getH3GamePath(config), modsInfo, "working")
									} catch (e) {
										failedMods = [...failedMods, [modFolder.name || "Unknown mod", String(e)]]
										await remove("working", { recursive: true })
										continue
									}

									const validation = await commands.rsValidateModFolder("working", true)

									if (validation.result === "pass") {
										await commands.rsCopyFolder("working", await join("Mods", JSON.parse(await readTextFile(await join("working", "manifest.json"))).id))
										gui(config).knownMods = [...gui(config).knownMods, JSON.parse(await readTextFile(await join("working", "manifest.json"))).id]
									} else {
										failedMods = [...failedMods, [modFolder.name || "Unknown mod", validation.message]]
									}

									await remove("working", { recursive: true })
								} catch (e) {
									failedMods = [...failedMods, [modFolder.name || "Unknown mod", String(e)]]
								}
							}

							batchModAddingDialog.hide()

							if (failedMods.length > 0) {
								failedModsDialog.show()
							}
						}
					}}
					>Import Entire Mods Folder
				</sl-button>
			{/if}
			<sl-input placeholder={m.FilterAvailableMods()} value={disabledModsFilter} oninput={(evt) => (disabledModsFilter = evt.target.value)}>
				<sl-icon name="funnel" slot="prefix"></sl-icon>
			</sl-input>
		</div>
		<div class="mt-4 pr-4">
			{#await Promise.all(allMods.map((a) => getModManifest(a))) then manifests}
				<Masonry
					style="justify-content: start"
					items={[
						...manifests
							.filter((a) => !config.deployOrder.includes(a.id))
							.filter((a) => (disabledModsFilter ? (loc(a.name) + loc(a.description)).toLowerCase().includes(disabledModsFilter.toLowerCase()) : true))
							.sort((a, b) => loc(a.name).localeCompare(loc(b.name))),
						null
					] as const}
					gap={15}
					getId={(a) => a?.id || "dummy"}
					virtualize
					height="calc(100vh - 15rem)"
					getEstimatedHeight={() => 240}
					order="row-first"
				>
					{#snippet children({ item: manifest })}
						{#if manifest}
							<sl-card class="w-full">
								<div slot="header">
									<h2 class="text-xl font-bold">{loc(manifest.name)} <span class="font-light">{manifest.version}</span></h2>
									<div class="text-sm">{manifest.authors.join(", ")}</div>
								</div>
								<div class="text-[0.95rem] 2xl:text-base">{loc(manifest.description)}</div>
								<div slot="footer" class="flex flex-wrap gap-2">
									{#await validateManifest then validateManifest}
										{#if validateManifest(manifest)}
											<sl-button variant="primary" onclick={() => config.deployOrder.push(manifest.id)}>{m.EnableButton()}</sl-button>
										{:else}
											<sl-button variant="danger" disabled title={m.InvalidModButtonDesc()}>{m.InvalidModButton()}</sl-button>
										{/if}
									{/await}
									<sl-button
										variant="danger"
										onclick={async () => {
											await remove(await join("Mods", manifest.id), { recursive: true })
											gui(config).knownMods = gui(config).knownMods.filter((a) => a !== manifest.id)
										}}>{m.DeleteButton()}</sl-button
									>
								</div>
							</sl-card>
						{:else}
							<div class="h-96"></div>
						{/if}
					{/snippet}
				</Masonry>
			{:catch e}
				<p>{m.ErrorGettingMods()}</p>
				<pre><code>{String(e)}</code></pre>
			{/await}
		</div>
	</div>
	<div>
		<h1 class="text-4xl 2xl:text-5xl font-bold mb-2">{m.EnabledMods()}</h1>
		<div class="flex flex-row flex-wrap gap-2">
			<sl-button variant={needToRedeploy ? "success" : "primary"} href={config.gamePath && "/deploy"} disabled={!config.gamePath}>{m.ApplyModsButton()}</sl-button>
			{#await gameInstalls then gameInstalls}
				<sl-select
					class="flex-grow"
					placeholder={m.GameInstall()}
					value={config.gamePath
						? gameInstalls.some((a) => a.path === config.gamePath)
							? gameInstalls.findIndex((a) => a.path === config.gamePath).toString()
							: `custom-${btoa(config.gamePath)}`
						: "none"}
					onsl-change={(e) => {
						if (e.target.value === "none") {
							config.gamePath = null
						} else if (e.target.value.startsWith("custom-")) {
							config.gamePath = atob(e.target.value.replace("custom-", ""))
						} else {
							config.gamePath = gameInstalls[parseInt(e.target.value)].path
						}
					}}
				>
					<sl-option value="none">{m.NoGameSelected()}</sl-option>
					{#each gameInstalls as install, i}
						<sl-option value={i} class="break-all">{install.path}</sl-option>
					{/each}
					{#if config.gamePath && !gameInstalls.some((a) => a.path === config.gamePath)}
						<sl-option value={`custom-${btoa(config.gamePath)}`} class="break-all">{config.gamePath}</sl-option>
					{/if}
				</sl-select>
			{:catch e}
				{e}
			{/await}
			<sl-input placeholder={m.FilterEnabledMods()} value={enabledModsFilter} oninput={(evt) => (enabledModsFilter = evt.target.value)}>
				<sl-icon name="funnel" slot="prefix"></sl-icon>
			</sl-input>
		</div>
		<SortableList
			class="mt-4 h-[calc(100vh-15rem)] overflow-y-auto pr-4"
			animation={150}
			forceFallback
			fallbackTolerance={10}
			onEnd={(evt) => {
				if (typeof evt.newIndex !== "undefined" && typeof evt.oldIndex !== "undefined") {
					config.deployOrder.splice(evt.newIndex, 0, config.deployOrder.splice(evt.oldIndex, 1)[0])
				}
			}}
		>
			{#each config.deployOrder as mod}
				{#await getModManifest(mod) then manifest}
					<sl-card
						class="mb-2 w-full cursor-grab select-none"
						style={enabledModsFilter && !(loc(manifest.name) + loc(manifest.description)).toLowerCase().includes(enabledModsFilter.toLowerCase())
							? "filter: brightness(0.75); transition: 250ms filter"
							: "transition: 250ms filter"}
					>
						<div class="-ml-2 flex gap-4 items-center">
							<sl-icon name="grip-vertical" class="flex-shrink-0 text-2xl text-neutral-600"></sl-icon>
							<div class="flex-grow flex flex-wrap 2xl:flex-nowrap gap-4 items-center">
								<div class="flex-grow">
									<h2 class="text-xl font-bold">{loc(manifest.name)} <span class="font-light">{manifest.version}</span></h2>
									<p class="text-sm">{loc(manifest.description)}</p>
								</div>
								<div class="flex gap-2 items-center">
									{#if manifest.options?.filter((a) => a.type !== "conditional").length}
										<sl-button variant={!isEqual(getAllOptions(manifest.options), gui(config).knownModOptions[mod]) ? "success" : "primary"} href="/mod-options?mod={mod}"
											>{m.SettingsButton()}</sl-button
										>
									{/if}
									<sl-button variant="danger" onclick={() => (config.deployOrder = config.deployOrder.filter((a) => a !== mod))}>{m.DisableButton()}</sl-button>
								</div>
							</div>
						</div>
					</sl-card>
				{/await}
			{/each}
		</SortableList>
	</div>
</div>

{#if showDropHint}
	<div transition:fade={{ duration: 100 }} class="w-screen h-screen absolute top-0 left-0 bg-black/90 flex justify-center items-center">
		<h1 class="text-4xl 2xl:text-5xl font-bold">{m.DropHint()}</h1>
	</div>
{/if}
