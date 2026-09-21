<script lang="ts">
	import { page } from "$app/state"
	import { evaluateCondition, getAllOptions, getModManifest, loc } from "$lib/mods.svelte"
	import OptionsSection from "./OptionsSection.svelte"
	import * as m from "$lib/paraglide/messages"
	import { marked } from "marked"
	import { sanitiseInline } from "$lib/utils"
	import { commands, type Manifest, type ModOption } from "$lib/bindings"
	import { convertFileSrc } from "@tauri-apps/api/core"
	import { config, validateConfig, type GUIConfig } from "$lib/config.svelte"
	import md5 from "md5"
	import Zoom from "svelte-medium-image-zoom"

	const mod = $derived(page.url.searchParams.get("mod") || "")
	const manifest = $derived(getModManifest(mod).then((m) => validateConfig(config).then(() => m)))

	let showOptions = $state(true)

	function findOption(opts: ModOption[], id: string): ModOption | null {
		for (const opt of opts) {
			if (opt.id === id) return opt
			if (opt.type === "optionGroup") {
				const found = findOption(opt.options, id)
				if (found) return found
			}
		}
		return null
	}

	async function applyPreset(values: Record<string, any> | null) {
		showOptions = false
		if (values === null) {
			config.modOptions[mod] = {}
		} else {
			for (const [opt, val] of Object.entries(values)) {
				const option = findOption((await manifest).options || [], opt)!
				if (option.type === "conditional" || option.type === "optionGroup") throw new Error("Invalid option in preset")
				config.modOptions[mod][opt] = {
					type: option.type,
					value: val
				}
			}
		}
		await validateConfig(config)
		showOptions = true
	}

	function getOptionHierarchy(manifest: Manifest) {
		const result: { id: string; parent: string | null; optionId: string | null; name: string; showWhenHidden: boolean; depth: number }[] = [
			{ id: "opt-top-level", parent: null, optionId: null, name: "Options", showWhenHidden: true, depth: 0 }
		]
		function recurse(parent: string, opts: ModOption[], depth = 1) {
			for (const opt of opts) {
				if (opt.type === "optionGroup") {
					result.push({
						id: `opt-${md5(opt.id)}`,
						parent,
						optionId: opt.id,
						name: loc(opt.name),
						showWhenHidden: !opt.displayCondition || !!opt.hiddenDescription,
						depth
					})
					recurse(`opt-${md5(opt.id)}`, opt.options, depth + 1)
				}
			}
		}
		recurse("opt-top-level", manifest.options || [])
		return result
	}

	const reqCache = $state({
		install: null as Awaited<ReturnType<typeof commands.rsDetectGame>> | null,
		enabledMods: {} as Record<string, string>
	})

	$effect(() => {
		if (config.gamePath) {
			void commands.rsDetectGame(config.gamePath).then((install) => {
				reqCache.install = install
			})
		}
	})

	$effect(() => {
		void Promise.all(config.deployOrder.map(async (a) => [a, (await getModManifest(a)).version])).then((enabledMods) => {
			reqCache.enabledMods = Object.fromEntries(enabledMods)
		})
	})

	let optionsVisibility: Record<string, boolean> = $state({})

	$effect(() => {
		manifest.then((manifest) => {
			;(config.gui as unknown as GUIConfig).knownModOptions[mod] = getAllOptions(manifest.options || [])
		})
	})

	let optionsDisplay: Record<string, boolean> = $state({})

	$effect(() => {
		manifest.then(async (manifest) => {
			const hierarchy = getOptionHierarchy(manifest)
			for (const section of hierarchy) {
				const option = section.optionId ? (findOption(manifest.options || [], section.optionId) as (ModOption & { type: "optionGroup" }) | null) : null
				const display = !(option && option.displayCondition) || (await evaluateCondition(manifest.id, option.displayCondition, reqCache.enabledMods, reqCache.install, config))
				function parents({ parent }: { parent: string | null }) {
					return parent ? [parent, ...parents(hierarchy.find((a) => a.id === parent)!)] : []
				}
				optionsDisplay[section.id] = display && parents(section).every((a) => optionsDisplay[a])
			}
		})
	})
</script>

{#await manifest then manifest}
	<h1 class="text-4xl 2xl:text-5xl font-bold mb-2">{loc(manifest.name)}</h1>
	{#if !config["PLACEHOLDER"] && manifest.options}
		<div class="flex flex-wrap gap-2 mb-4">
			<sl-button variant="primary" onclick={() => void applyPreset(null)}>{m.ResetToDefaultsButton()}</sl-button>
			{#each manifest.presets || [] as preset}
				<sl-tooltip placement="bottom">
					<sl-button variant="primary" onclick={() => void applyPreset(preset.values)}>{loc(preset.name)}</sl-button>
					<div slot="content">
						{#if preset.image}
							{#await (async () => (preset.image ? commands.rsResolveModRelativePath(mod, preset.image) : ""))() then imagePath}
								<Zoom>
									<img class="mb-2" src={convertFileSrc(imagePath)} alt="" />
								</Zoom>
							{/await}
						{/if}
						<p>
							{#await marked(loc(preset.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
						</p>
					</div>
				</sl-tooltip>
			{/each}
		</div>
		{#if showOptions}
			{@const sections = getOptionHierarchy(manifest)}
			<div class="flex gap-16 w-full">
				{#if sections.filter((a) => a.showWhenHidden).length >= 5}
					<div class="hidden xl:block w-max flex-shrink-0 h-[calc(100vh-14rem)] overflow-y-auto text-lg">
						<ul>
							{#each sections as section, idx (idx)}
								<li
									{@attach (el) => {
										let className = "flex"
										if (
											Object.entries(optionsVisibility)
												.filter((a) => a[1])
												.sort((a, b) => sections.findIndex((s) => s.id === a[0]) - sections.findIndex((s) => s.id === b[0]))?.[0]?.[0] === section.id
										) {
											className += " font-bold"
										}
										if (section.optionId) {
											if (optionsDisplay[section.id]) {
												className += " hover:text-neutral-300"
											} else if (section.showWhenHidden) {
												className += " hover:text-neutral-500 text-neutral-400"
											} else {
												className += " hidden"
											}
										} else {
											className += " hover:text-neutral-300"
										}
										el.className = className
									}}
								>
									{#each new Array(section.depth).keys()}
										<div class="border-l border-l-neutral-700 pl-4"></div>
									{/each}
									<a
										href="#"
										onclick={() => {
											const el = document.getElementById(section.id)
											if (el) {
												el.scrollIntoView({ behavior: "smooth" })
											}
										}}
									>
										{section.name}
									</a>
								</li>
							{/each}
						</ul>

						<!-- So that the layout doesn't shift when the rightmost item is bolded -->
						<div class="opacity-0 font-bold select-none" aria-hidden={true} style="padding-left: {Math.max(...sections.map((a) => a.depth))}rem"
							>{sections.filter((a) => a.depth === Math.max(...sections.map((a) => a.depth))).sort((a, b) => b.name.length - a.name.length)[0]}</div
						>
					</div>
				{/if}
				<div class="flex-grow h-[calc(100vh-14rem)] overflow-y-auto pr-4 text-[0.95rem] 2xl:text-base" id="options-container">
					<OptionsSection
						{manifest}
						onVisible={(id, visible) => {
							optionsVisibility[id] = visible
						}}
						{reqCache}
						clonedConfig={JSON.parse(JSON.stringify(config))}
						{applyPreset}
					/>
				</div>
			</div>
		{/if}
	{:else}
		{m.NoOptionsForMod()}
	{/if}
{/await}

<style>
	:global(.markdown ul li) {
		list-style-type: disc;
		list-style-position: inside;
	}

	:global(.markdown ol li) {
		list-style-type: decimal;
		list-style-position: inside;
	}

	:global(.markdown p:not(:nth-of-type(1))) {
		@apply mt-1;
	}
</style>
