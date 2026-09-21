<script lang="ts">
	import "@xterm/xterm/css/xterm.css"
	import { onDestroy, onMount } from "svelte"
	import { commands, type DeployIPCMessage, events } from "$lib/bindings"
	import { updateConfig } from "$lib/config.svelte"
	import { exists } from "@tauri-apps/plugin-fs"
	import * as m from "$lib/paraglide/messages"
	import { getModManifest, loc } from "$lib/mods.svelte"
	import type { UnlistenFn } from "@tauri-apps/api/event"
	import { goto } from "$app/navigation"
	import { getCurrentWindow } from "@tauri-apps/api/window"
	import { config } from "$lib/config.svelte"
	import { Terminal } from "@xterm/xterm"
	import { FitAddon } from "@xterm/addon-fit"
	import { WebglAddon } from "@xterm/addon-webgl"

	let deployProcError: string | undefined = $state()
	let deployStarted = $state(false)
	let deployFinished = $state(false)
	let deploySuccess = $state(true)
	let deployOutput: string[] = $state([])
	let terminalElement: HTMLDivElement | undefined = $state()

	const unlisteners: UnlistenFn[] = []

	onDestroy(() => {
		for (const unlistener of unlisteners) {
			unlistener()
		}
	})

	const fit = new FitAddon()

	onMount(async () => {
		try {
			const terminal = new Terminal({
				// Qualia by u/starlig-ht, slightly modified for background colour
				theme: {
					black: "#101010",
					red: "#EFA6A2",
					green: "#80C990",
					yellow: "#C8C874",
					blue: "#A3B8EF",
					magenta: "#E6A3DC",
					cyan: "#50CACD",
					white: "#F4F4F4",
					brightBlack: "#878787",
					brightRed: "#E0AF85",
					brightGreen: "#5ACCAF",
					brightYellow: "#C8C874",
					brightBlue: "#CCACED",
					brightMagenta: "#F2A1C2",
					brightCyan: "#74C3E4",
					brightWhite: "#C0C0C0",
					foreground: "#F4F4F4",
					background: "#262626"
				},
				fontFamily: "Fira Code Variable",
				convertEol: true,
				disableStdin: true,
				scrollback: 10000
			})
			terminal.loadAddon(fit)
			const addon = new WebglAddon()
			addon.onContextLoss(() => {
				addon.dispose()
			})
			terminal.loadAddon(addon)
			terminal.open(terminalElement!)

			unlisteners.push(
				await events.deployOutput.listen(({ payload }) => {
					terminal.write((deployOutput.length === 0 ? "" : "\n") + payload)
					deployOutput = [...deployOutput, ...payload.split("\n")]
				})
			)

			unlisteners.push(
				await events.deployError.listen(({ payload }) => {
					deployProcError = payload
				})
			)

			unlisteners.push(
				await events.deployExitCode.listen(async ({ payload }) => {
					await updateConfig()

					if (payload !== 0) {
						deploySuccess = false
					}

					deployFinished = true
				})
			)

			unlisteners.push(
				await events.deployIpcMessage.listen(({ payload }) => {
					switch (payload.type) {
						case "diagnostic":
							diagnostics = [...diagnostics, payload.data]
							break
						case "unrecognisedGameVersion":
							getCurrentWindow()
								.maximize()
								.then(() => goto("/unrecognised-game-version"))
							break
						case "progressStart":
							progress = { id: payload.data.id, name: payload.data.name, current: 0, max: payload.data.max }
							break
						case "spinnerStart":
							spinners = [
								...spinners,
								{
									id: payload.data.id,
									prefix: payload.data.prefix,
									target: payload.data.target,
									name: payload.data.name
								}
							]
							break
						case "progressAdvance":
							progress!.current += payload.data.amount
							break
						case "progressFinish":
							if (payload.data.id === progress?.id) {
								progress = null
							} else {
								spinners = spinners.filter((a) => a.id !== payload.data.id)
							}
							break
					}
				})
			)

			for (const mod of config.deployOrder) {
				const manifest = await getModManifest(mod)
				if (manifest) {
					modManifests[mod] = manifest
				}
			}

			deployStarted = true

			await commands.rsDeploy()
		} catch (e) {
			deployProcError = String(e)
		}
	})

	let diagnostics: (DeployIPCMessage & { type: "diagnostic" })["data"][] = $state([])

	function computeDiagnostics(diagnostics: (DeployIPCMessage & { type: "diagnostic" })["data"][]) {
		const diags: { id: string; target: string; message: string }[] = []

		const modId = (target) => (target.type === "mod" || target.type === "operation" ? target.data.modId : null)

		for (const target of diagnostics
			.filter((a) => diagnostics.filter((diag) => modId(a.target) === modId(diag.target)).length >= 3)
			.map((a) => modId(a.target))
			.filter((a, i, arr) => arr.indexOf(a) === i)) {
			diags.push({
				id: `${target}-warnings`,
				target: loc(modManifests[target].name),
				message: `${diagnostics.filter((diag) => modId(diag.target) === target).length} warnings produced; see log for details.`
			})
		}

		for (const [idx, diag] of diagnostics.entries()) {
			if (diag.target.type !== "none" && diagnostics.filter((a) => modId(a.target) === modId(diag.target)).length >= 3) {
				continue
			}

			diags.push({
				id: idx.toString(),
				target: diag.target.type === "none" ? "Main" : loc(modManifests[modId(diag.target)].name),
				message: diag.message
			})
		}

		return diags
	}

	const stripAnsi = (input: string) => input.replaceAll(/[\u001b\u009b][[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-ORZcf-nqry=><]/g, "")

	const modManifests = {}

	function computeErrors(deployOutput: string[]) {
		let errors: { id: string; target: string; message: string }[] = []

		for (const [idx, line] of deployOutput.filter((a) => a.match(/.*Error.*\t/)).entries()) {
			const target = stripAnsi(line)
				.match(/Error\t(.*?)\t/)![1]
				.trim()

			const modID = target.match(/^(.*?) - /)?.[1]

			errors.push({
				id: idx.toString(),
				target: modID ? loc(modManifests[modID].name) : target,
				message: stripAnsi(line)
					.replace(/.*Error\t.*?\t/, "")
					.trim()
			})
		}

		return errors
	}

	let progress: { id: number; name: string; current: number; max: number } | null = $state(null)
	let spinners: { id: number; prefix: string; target: string | null; name: string }[] = $state([])

	export const ro = (node: HTMLElement, callback: (entry: ResizeObserverEntry) => unknown) => {
		const ro = new ResizeObserver(([entry]) => callback(entry))
		ro.observe(node)
		return {
			destroy: () => ro.disconnect()
		}
	}
</script>

<h1 class="mt-12 text-4xl 2xl:text-5xl font-bold mb-2">{m.ApplyingMods()}</h1>
<subtitle>{m.ApplyingModsDesc()}</subtitle>
<div class="mt-2 grid grid-cols-3 gap-4 text-[0.95rem] 2xl:text-base">
	<div style="height: calc(100vh - 18rem)" class="flex flex-col col-span-2">
		<div
			use:ro={({ contentRect }) => {
				if (!terminalElement) return
				terminalElement.style.height = contentRect.height + "px"
				terminalElement.style.width = contentRect.width + "px"
			}}
			class="flex-grow flex-shrink overflow-hidden p-4 bg-[#262626]"
		>
			<div
				use:ro={() => {
					fit.fit()
				}}
				bind:this={terminalElement}
			></div>
		</div>
	</div>
	<div style="height: calc(100vh - 18rem)" class="overflow-y-auto pr-4">
		{#if !deployFinished}
			<div class="flex flex-col max-h-[15vh] has-[*]:mb-4">
				{#if progress}
					<div class="flex gap-4 items-center">
						<span class="font-semibold">{progress.name}</span>
						<sl-progress-bar class="flex-grow" value={(progress.current / progress.max) * 100}></sl-progress-bar>
					</div>
				{/if}
				{#if spinners.length}
					<div class:mt-2={!!progress} class="flex-grow flex flex-col gap-2 overflow-y-auto">
						{#each spinners.slice(0, 15) as spinner}
							<div class="flex items-center mr-2">
								<div class="flex-grow">
									{#if spinner.target}
										<span class="font-semibold">{spinner.prefix}<span class="pl-4">{spinner.target}</span></span>
										<br /><span class="break-all">{spinner.name}</span>
									{:else}
										<span class="font-semibold">{spinner.prefix}</span> {spinner.name}
									{/if}
								</div>
								<div class="ml-4">
									<sl-spinner></sl-spinner>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
		{#if deployStarted}
			<sl-alert class="mb-4" variant="primary" open>
				<sl-icon slot="icon" name="info-circle"></sl-icon>
				<strong>{m.DeploymentStarted()}</strong><br />
				{m.DeploymentStartedDesc()}
			</sl-alert>
		{/if}
		{#if deployProcError}
			<sl-alert class="mb-4" variant="danger" open>
				<sl-icon slot="icon" name="exclamation-octagon"></sl-icon>
				<strong>{m.CouldntStartDeployment()}</strong><br />
				{m.CouldntStartDeploymentDesc()}
				<div><code class="break-all">{deployProcError}</code></div>
			</sl-alert>
		{/if}
		{#each computeDiagnostics(diagnostics) as diagnostic (diagnostic.id)}
			<sl-alert class="mb-4" variant="warning" open>
				<sl-icon slot="icon" name="exclamation-triangle"></sl-icon>
				<strong>{diagnostic.target === "Main" ? m.Warning() : diagnostic.target}</strong><br />
				{diagnostic.message}
			</sl-alert>
		{/each}
		{#each computeErrors(deployOutput) as error (error.id)}
			<sl-alert class="mb-4" variant="danger" open>
				<sl-icon slot="icon" name="exclamation-octagon"></sl-icon>
				<strong>{error.target === "Main" ? m.Error() : error.target}</strong><br />
				{error.message}
			</sl-alert>
		{/each}
		{#if deployFinished && deploySuccess && !deployOutput.some((a) => a.trim().startsWith("Error:") || a.match(/.*(Error|Warning).*\t/))}
			<sl-alert class="mb-4" variant="success" open>
				<sl-icon slot="icon" name="check2-circle"></sl-icon>
				<strong>{m.DeploymentSuccessful()}</strong><br />
				{m.DeploymentSuccessfulDesc()}
			</sl-alert>
		{/if}
	</div>
</div>
{#if deployFinished}
	{@const deployFailed = !deploySuccess || deployOutput.some((a) => a.trim().startsWith("Error:") || a.match(/.*Error.*\t/))}
	{#if deployFailed}
		<div class="my-4 text-red-300">{m.DeploymentUnsuccessful()}</div>
	{:else if deployOutput.some((a) => a.match(/.*Warning.*\t/))}
		<div class="my-4 text-yellow-300">{m.IssuesInDeployment()}</div>
	{:else}
		<div class="my-4 text-green-300">{m.DeploymentSuccessful()}</div>
	{/if}
	<div class="flex flex-wrap gap-2">
		<sl-button variant="primary" href="/mods">{m.ContinueButton()}</sl-button>
		{#if deployFailed}
			{#await exists("debug") then profilePresent}
				{#if profilePresent}
					<sl-button variant="warning" onclick={() => void commands.rsSaveDebugProfile()}>{m.SaveDebugProfileButton()}</sl-button>
				{/if}
			{/await}
		{/if}
	</div>
{/if}
