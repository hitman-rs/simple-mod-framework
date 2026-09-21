<script lang="ts">
	import * as m from "$lib/paraglide/messages"
	import { commands, type GlacierGame } from "$lib/bindings"
	import { config } from "$lib/config.svelte"
	import { join, resolve } from "@tauri-apps/api/path"
	import { exists, remove, writeTextFile } from "@tauri-apps/plugin-fs"
	import { download } from "@tauri-apps/plugin-upload"
	import { gt } from "semver"
	import { versionCache } from "$lib/ephemeral.svelte"

	let dummy = $state(0)
	let downloadProgress = $state(0)

	async function installSDK(version: GlacierGame, latestVersion: string) {
		if (!config.gamePath) return

		const url = await commands.rsGetLatestSdkDownloadUrl(version, !latestVersion.includes("."))
		downloadProgress = 0.001
		await download(url, await resolve("tempArchive"), ({ progressTotal, total }) => (downloadProgress = (progressTotal / total) * 100))

		if (version === "fl") {
			if (await exists("working")) {
				await remove("working", { recursive: true })
			}

			await commands.rsExtractArchive("tempArchive", "working")
			await commands.rsCopyFolder(await join("working", "Retail"), config.gamePath)
			await remove("working", { recursive: true })
		} else {
			await commands.rsExtractArchive("tempArchive", config.gamePath)
		}

		await writeTextFile(await join(config.gamePath, "sdk.txt"), latestVersion)

		await remove(await resolve("tempArchive"))
		downloadProgress = 0
		dummy = Math.random()
	}
</script>

<sl-card>
	{#key dummy}
		{#if config.gamePath}
			{#await commands.rsDetectGame(config.gamePath) then gameInstall}
				{#if gameInstall && ["h3", "fl"].includes(gameInstall?.version || "")}
					{#await commands.rsGetInstalledSdkVersion(config.gamePath) then installed}
						{#if installed}
							{@const isArtifact = !installed.includes(".")}
							{@const latestVersion = versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`]
								? Promise.resolve(versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`])
								: (isArtifact ? commands.rsGetLatestSdkArtifact(gameInstall.version) : commands.rsGetLatestSdkRelease(gameInstall.version)).then((ver) => {
										versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`] = ver
										return ver
									})}

							<div class="text-lg font-semibold">
								{m.Installed()}
								<span class="font-light"
									>({m.Version({ version: isArtifact ? installed.slice(0, 6) : installed })},
									{#await latestVersion}
										{m.CheckingForUpdatesParenthetical()}{:then latest}
										{#if isArtifact ? latest !== installed : gt(latest, installed)}
											{m.UpdateAvailableParenthetical({ version: isArtifact ? latest.slice(0, 6) : latest })}{:else}
											{m.UpToDateParenthetical()}{/if}{:catch}
										{m.CantCheckUpdatesParenthetical()}{/await})
								</span>
							</div>
							<p>{m.SDKInstalledDesc()}</p>
							{#if downloadProgress === 0}
								<div class="mt-4 flex flex-wrap gap-2">
									{#await latestVersion then latest}
										{#if isArtifact ? latest !== installed : gt(latest, installed)}
											<sl-button variant="primary" onclick={() => installSDK(gameInstall.version, latest)}>{m.UpdateButton()}</sl-button>
										{/if}
									{/await}
									{#if isArtifact}
										{#if gameInstall.version !== "fl"}
											<!-- No releases yet for ZKntSDK -->
											<sl-button
												variant="primary"
												onclick={async () => {
													const latestVersion = versionCache.versions[`sdk-${gameInstall.version}-release`]
														? versionCache.versions[`sdk-${gameInstall.version}-release`]
														: await commands.rsGetLatestSdkRelease(gameInstall.version).then((ver) => {
																versionCache.versions[`sdk-${gameInstall.version}-release`] = ver
																return ver
															})

													await installSDK(gameInstall.version, latestVersion)
												}}>{m.SwitchToStable()}</sl-button
											>
										{/if}
									{:else}
										<sl-button
											variant="primary"
											onclick={async () => {
												const latestVersion = versionCache.versions[`sdk-${gameInstall.version}-artifact`]
													? versionCache.versions[`sdk-${gameInstall.version}-artifact`]
													: await commands.rsGetLatestSdkArtifact(gameInstall.version).then((ver) => {
															versionCache.versions[`sdk-${gameInstall.version}-artifact`] = ver
															return ver
														})

												await installSDK(gameInstall.version, latestVersion)
											}}>{m.SwitchToUnstable()}</sl-button
										>
									{/if}
									<sl-button
										variant="danger"
										onclick={async () => {
											if (!config.gamePath) return

											await remove(await join(config.gamePath, "dinput8.dll"))

											if (await exists(await join(config.gamePath, "ZHMModSDK.dll"))) {
												await remove(await join(config.gamePath, "ZHMModSDK.dll"))
											}

											if (await exists(await join(config.gamePath, "ZKntSdk.dll"))) {
												await remove(await join(config.gamePath, "ZKntSdk.dll"))
											}

											if (await exists(await join(config.gamePath, "sdk.txt"))) {
												await remove(await join(config.gamePath, "sdk.txt"))
											}

											dummy = Math.random()
										}}>{m.RemoveButton()}</sl-button
									>
								</div>
							{:else}
								<div class="mt-4">{m.Downloading()}</div>
								<sl-progress-bar class="mt-2" value={downloadProgress}></sl-progress-bar>
							{/if}
						{:else}
							<div class="text-lg font-semibold">{m.NotInstalled()}</div>
							<p>{m.SDKNotInstalledDesc()}</p>
							{#if downloadProgress === 0}
								<div class="mt-4 flex flex-wrap gap-2">
									<sl-button
										variant="primary"
										onclick={async () => {
											// No releases yet for ZKntSDK
											const isArtifact = gameInstall.version === "fl"

											const latest = versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`]
												? versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`]
												: await (isArtifact ? commands.rsGetLatestSdkArtifact(gameInstall.version) : commands.rsGetLatestSdkRelease(gameInstall.version)).then((ver) => {
														versionCache.versions[`sdk-${gameInstall.version}-${isArtifact ? "artifact" : "release"}`] = ver
														return ver
													})

											await installSDK(gameInstall.version, latest)
										}}>{m.InstallButton()}</sl-button
									>
								</div>
							{:else}
								<div class="mt-4">{m.Downloading()}</div>
								<sl-progress-bar class="mt-2" value={downloadProgress}></sl-progress-bar>
							{/if}
						{/if}
					{:catch}
						{m.UnableCheckSDKDesc()}
					{/await}
				{:else}
					{m.SDKUnsupported()}
				{/if}
			{:catch e}
				{e}
			{/await}
		{:else}
			{m.SDKSelectGamePath()}
		{/if}
	{/key}
</sl-card>
