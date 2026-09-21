<script lang="ts">
	import { commands } from "$lib/bindings"
	import type { Config, Manifest, ManifestConditions, ModOption } from "$lib/bindings"
	import { config } from "$lib/config.svelte"
	import { convertFileSrc } from "@tauri-apps/api/core"
	import ValidatedNumberInput from "./ValidatedNumberInput.svelte"
	import * as m from "$lib/paraglide/messages"
	import { evaluateCondition, getModManifest, loc } from "$lib/mods.svelte"
	import { satisfies } from "semver"
	import { marked } from "marked"
	import { observe, sanitiseInline } from "$lib/utils"
	import ValidatedStringInput from "./ValidatedStringInput.svelte"
	import Zoom from "svelte-medium-image-zoom"

	interface Props {
		option: ModOption
		manifest: Manifest
		colSize: number
		reqCache: {
			install: { version: string; platform: string } | null
			enabledMods: Record<string, string>
		}
		clonedConfig: Config
	}

	let { option, manifest, colSize, reqCache, clonedConfig }: Props = $props()

	async function checkConditions(conditions: ManifestConditions, config: Config, enabledMods: Record<string, string>, install: { version: string; platform: string } | null) {
		if (install && conditions.supportedGames?.length) {
			if (
				!conditions.supportedGames.some((a) => {
					if (a === "h1") return install.version === "h1"
					if (a === "h2") return install.version === "h2"
					if (a === "h3") return install.version === "h3"
					if (a === "fl") return install.version === "fl"
					return install.version === a.split("-")[0] && install.platform === a.split("-")[1]
				})
			) {
				return m.OnlySupports({
					supported: conditions.supportedGames
						.map((a) => {
							let game = { h1: "HITMAN™", h2: "HITMAN 2", h3: "HITMAN 3", fl: "007 First Light" }[a.split("-")[0]]
							if (a.split("-").length === 2) {
								game += ` (${{ steam: "Steam", epic: "Epic", microsoft: "Microsoft", gog: "GOG" }[a.split("-")[1]]})`
							}
							return game
						})
						.join(", ")
				})
			}
		}

		if (conditions.requiredMods?.length) {
			for (const req of conditions.requiredMods) {
				const reqId = (req as unknown as string).split("@")[0]
				const reqVersion = (req as unknown as string).split("@")[1]

				if (!(config.deployOrder.includes(reqId) && (!reqVersion || satisfies((await getModManifest(reqId)).version, reqVersion)))) {
					return m.Requires({ mod: req })
				}
			}
		}

		if (conditions.incompatibleMods?.length) {
			for (const inc of conditions.incompatibleMods) {
				const incId = (inc as unknown as string).split("@")[0]
				const incVersion = (inc as unknown as string).split("@")[1]

				if (config.deployOrder.includes(incId) && (!incVersion || satisfies((await getModManifest(incId)).version, incVersion))) {
					return m.IncompatibleWith({ mod: inc })
				}
			}
		}

		if (conditions.requiredConditions?.length) {
			for (const cond of conditions.requiredConditions) {
				if (!(await evaluateCondition(manifest.id, cond.condition, enabledMods, install, config))) {
					return loc(cond.explanation)
				}
			}
		}

		if (conditions.incompatibleConditions?.length) {
			for (const cond of conditions.incompatibleConditions) {
				if (!(await evaluateCondition(manifest.id, cond.condition, enabledMods, install, config))) {
					return loc(cond.explanation)
				}
			}
		}

		return null
	}

	let currentHeight = $state(0)
	let height = $state(300)
	let visible = $state(false)
	let rendering = $state(false)

	$effect(() => {
		if (currentHeight > 0) {
			height = currentHeight
		}
	})
</script>

<div
	style="width: {colSize}px"
	use:observe={{
		root: document.getElementById("options-container"),
		rootMargin: "300% 0px",
		callback: (_, intersecting) => {
			visible = intersecting
		}
	}}
	use:observe={{
		root: document.getElementById("options-container"),
		rootMargin: "600% 0px",
		callback: (_, intersecting) => {
			rendering = intersecting
		}
	}}
>
	{#if rendering}
		<div style="min-height: {height}px">
			<div class:hidden={!visible} bind:clientHeight={currentHeight}>
				{#if option.type === "boolean"}
					<sl-card class="w-full">
						{#if option.image}
							{#await (async () => (option.image ? commands.rsResolveModRelativePath(manifest.id, option.image) : ""))() then imagePath}
								<div slot="image">
									<Zoom>
										<img loading="lazy" src={convertFileSrc(imagePath)} alt="" />
									</Zoom>
								</div>
							{/await}
						{/if}
						<div class="flex gap-1">
							<sl-checkbox
								checked={config.modOptions[manifest.id]![option.id]!.value}
								onsl-change={(evt) => {
									if (config.modOptions[manifest.id]![option.id]!.value !== evt.target.checked) {
										config.modOptions[manifest.id]![option.id]!.value = evt.target.checked
									}
								}}
								{@attach (el) =>
									checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
										el.disabled = problem !== null
										if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
											config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
										}
									})}
							></sl-checkbox>
							<div>
								<h3 class="mt-0.5 text-lg font-bold leading-tight">{loc(option.name)}</h3>
								<div class="mt-1 text-sm markdown">
									{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
								</div>
								{#await checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
									{#if problem !== null}
										<p class="mt-1 text-sm text-neutral-400">{problem}</p>
									{/if}
								{/await}
								<p class="mt-1 text-sm text-neutral-400">{option.defaultValue ? m.EnabledByDefault() : m.DisabledByDefault()}</p>
							</div>
						</div>
					</sl-card>
				{:else if option.type === "selection"}
					<sl-card class="w-full">
						<h3 class="text-lg font-bold leading-tight">{loc(option.name)}</h3>
						<p class="mt-1 text-sm mb-3">
							{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
						</p>
						{#await checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
							{#if problem !== null}
								<p class="mt-1 text-sm text-neutral-400">{problem}</p>
							{/if}
						{/await}
						{#if option.options.length < 7}
							<sl-radio-group
								value={config.modOptions[manifest.id]![option.id]!.value}
								onsl-change={(evt) => {
									if (config.modOptions[manifest.id]![option.id]!.value !== evt.target.value) {
										config.modOptions[manifest.id]![option.id]!.value = evt.target.value
									}
								}}
							>
								{#each option.options as subOption}
									<sl-radio
										value={subOption.id}
										class="mb-1"
										{@attach (el) =>
											checkConditions(subOption.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
												el.disabled = problem !== null
												if (problem !== null && config.modOptions[manifest.id]![option.id]!.value === subOption.id) {
													config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
												}
											})}
										{@attach (el) =>
											checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
												// Can't disable a radio group so disable all options instead
												el.disabled = problem !== null
												if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
													config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
												}
											})}
									>
										<div class="xl:max-w-[12vw]">
											<div class="mt-[3px] mb-2">
												<h3 class="text-lg font-bold leading-tight">
													{loc(subOption.name)}
													{#if subOption.id === option.defaultValue}
														<span class="text-sm font-normal text-neutral-400">{m.DefaultParenthetical()}</span>
													{/if}
												</h3>
												<p class="mt-1 text-sm">
													{#await marked(loc(subOption.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
												</p>
												{#await checkConditions(subOption.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
													{#if problem !== null}
														<p class="mt-1 text-sm text-neutral-400">{problem}</p>
													{/if}
												{/await}
											</div>
											{#if subOption.image}
												{#await (async () => (subOption.image ? commands.rsResolveModRelativePath(manifest.id, subOption.image) : ""))() then imagePath}
													<Zoom>
														<img loading="lazy" class="mb-2" src={convertFileSrc(imagePath)} alt="" />
													</Zoom>
												{/await}
											{/if}
										</div>
									</sl-radio>
								{/each}
							</sl-radio-group>
						{:else}
							<sl-select
								value={config.modOptions[manifest.id]![option.id]!.value}
								onsl-change={(evt) => {
									if (config.modOptions[manifest.id]![option.id]!.value !== evt.target.value) {
										config.modOptions[manifest.id]![option.id]!.value = evt.target.value
									}
								}}
								{@attach (el) =>
									checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
										el.disabled = problem !== null
										if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
											config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
										}
									})}
							>
								{#each option.options as subOption}
									<sl-option
										value={subOption.id}
										{@attach (el) =>
											checkConditions(subOption.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
												el.disabled = problem !== null
												if (problem !== null && config.modOptions[manifest.id]![option.id]!.value === subOption.id) {
													config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
												}
											})}
									>
										<div class="mt-1 mb-1">
											<p class="font-bold leading-tight">
												{loc(subOption.name)}
												<!-- So that the text after is hidden in the sl-select display -->
												<span class="hidden">{[...new Array(500).keys()].map(() => " ").join("")}</span>
												{#if subOption.id === option.defaultValue}
													<span class="text-sm font-normal">{m.DefaultParenthetical()}</span>
												{/if}
											</p>
											<p class="mt-1 text-sm">
												{#await marked(loc(subOption.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
											</p>
											{#await checkConditions(subOption.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
												{#if problem !== null}
													<p class="mt-1 text-sm text-neutral-400">{problem}</p>
												{/if}
											{/await}
										</div>
										{#if subOption.image}
											{#await (async () => (subOption.image ? commands.rsResolveModRelativePath(manifest.id, subOption.image) : ""))() then imagePath}
												<img loading="lazy" class="mt-2 mb-1" src={convertFileSrc(imagePath)} alt="" />
											{/await}
										{/if}
									</sl-option>
								{/each}
							</sl-select>
							{@const selectedOption = option.options.find((a) => a.id === config.modOptions[manifest.id]![option.id]!.value)}
							<p class="mt-3 text-sm">
								{#await marked(loc(selectedOption?.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
							</p>
							{#if selectedOption?.image}
								{#await (async () => (selectedOption.image ? commands.rsResolveModRelativePath(manifest.id, selectedOption.image) : ""))() then imagePath}
									<Zoom>
										<img loading="lazy" class="mt-2" src={convertFileSrc(imagePath)} alt="" />
									</Zoom>
								{/await}
							{/if}
						{/if}
					</sl-card>
				{:else if option.type === "number"}
					<sl-card class="w-full">
						{#if option.image}
							{#await (async () => (option.image ? commands.rsResolveModRelativePath(manifest.id, option.image) : ""))() then imagePath}
								<div slot="image">
									<Zoom>
										<img loading="lazy" src={convertFileSrc(imagePath)} alt="" />
									</Zoom>
								</div>
							{/await}
						{/if}
						<div>
							<h3 class="text-lg font-bold leading-tight">{loc(option.name)}</h3>
							<p class="mt-1 text-sm mb-3">
								{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
							</p>
							{#await checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
								{#if problem !== null}
									<p class="mt-1 text-sm text-neutral-400">{problem}</p>
								{/if}
								<ValidatedNumberInput
									bind:value={config.modOptions[manifest.id]![option.id]!.value as number}
									validation={option.validation || {}}
									disabled={problem !== null}
									{@attach (_) =>
										checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
											if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
												config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
											}
										})}
								/>
							{/await}
							<p class="mt-2 text-sm text-neutral-400">{m.DefaultValue()} {option.defaultValue}</p>
						</div>
					</sl-card>
				{:else if option.type === "color"}
					<sl-card class="w-full">
						{#if option.image}
							{#await (async () => (option.image ? commands.rsResolveModRelativePath(manifest.id, option.image) : ""))() then imagePath}
								<div slot="image">
									<Zoom>
										<img loading="lazy" src={convertFileSrc(imagePath)} alt="" />
									</Zoom>
								</div>
							{/await}
						{/if}
						<div>
							<div class="flex gap-3">
								<sl-color-picker
									size="small"
									value={config.modOptions[manifest.id]![option.id]!.value}
									opacity={!!option.alpha}
									onsl-change={(evt) => {
										config.modOptions[manifest.id]![option.id]!.value = evt.target.value
									}}
									swatches={option.defaultValue}
									{@attach (el) =>
										checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
											el.disabled = problem !== null
											if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
												config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
											}
										})}
								></sl-color-picker>
								<div class="mt-1">
									<h3 class="text-lg font-bold leading-tight">{loc(option.name)}</h3>
									<p class="mt-1 text-sm">
										{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
									</p>
								</div>
							</div>
						</div>
					</sl-card>
				{:else if option.type === "string"}
					<sl-card class="w-full">
						{#if option.image}
							{#await (async () => (option.image ? commands.rsResolveModRelativePath(manifest.id, option.image) : ""))() then imagePath}
								<div slot="image">
									<Zoom>
										<img loading="lazy" src={convertFileSrc(imagePath)} alt="" />
									</Zoom>
								</div>
							{/await}
						{/if}
						<div>
							<h3 class="text-lg font-bold leading-tight">{loc(option.name)}</h3>
							<p class="mt-1 text-sm mb-3">
								{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
							</p>
							{#await checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install) then problem}
								{#if problem !== null}
									<p class="mt-1 text-sm text-neutral-400">{problem}</p>
								{/if}
								<ValidatedStringInput
									bind:value={config.modOptions[manifest.id]![option.id]!.value as string}
									validation={option.validation || {}}
									disabled={problem !== null}
									{@attach (el) =>
										checkConditions(option.conditions || {}, clonedConfig, reqCache.enabledMods, reqCache.install).then((problem) => {
											if (problem !== null && config.modOptions[manifest.id]![option.id]!.value) {
												config.modOptions[manifest.id]![option.id]!.value = option.defaultValue
											}
										})}
								/>
							{/await}
							{#if option.defaultValue != ""}
								<p class="mt-2 text-sm text-neutral-400">{m.DefaultValue()} {option.defaultValue}</p>
							{/if}
						</div>
					</sl-card>
				{/if}
			</div>
		</div>
	{:else}
		<div style="height: {height}px"></div>
	{/if}
</div>

<style>
	:global([data-smiz-ghost]) {
		display: none;
	}
</style>
