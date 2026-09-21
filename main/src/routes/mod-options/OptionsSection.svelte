<script lang="ts">
	import { commands } from "$lib/bindings"
	import type { Config, Manifest, ModOption } from "$lib/bindings"
	import OptionsSection from "./OptionsSection.svelte"
	import { evaluateCondition, loc } from "$lib/mods.svelte"
	import { marked } from "marked"
	import { observe, sanitiseInline } from "$lib/utils"
	import md5 from "md5"
	import OptionsDisplay from "./OptionsDisplay.svelte"
	import { m } from "$lib/paraglide/messages"
	import { convertFileSrc } from "@tauri-apps/api/core"
	import Zoom from "svelte-medium-image-zoom"

	interface Props {
		manifest: Manifest
		id?: string
		onVisible?: (id: string, visible: boolean) => any
		reqCache: {
			install: { version: string; platform: string } | null
			enabledMods: Record<string, string>
		}
		clonedConfig: Config
		applyPreset: (values: Record<string, any> | null) => void
	}

	let { manifest, id, onVisible = () => {}, reqCache, clonedConfig, applyPreset }: Props = $props()

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

	const options = $derived(id ? (findOption(manifest.options || [], id) as { options: ModOption[] }).options : manifest.options || [])

	async function shouldDisplay(config: Config, enabledMods: Record<string, string>, install: { version: string; platform: string } | null) {
		if (!id) return true
		const option = findOption(manifest.options || [], id) as (ModOption & { type: "optionGroup" }) | null
		if (!option) return true
		if (option.displayCondition) {
			return await evaluateCondition(manifest.id, option.displayCondition, enabledMods, install, config)
		}
		return true
	}

	let display = $state(false)
	let displayError: string | null = $state(null)
	$effect(() => {
		shouldDisplay(clonedConfig, reqCache.enabledMods, reqCache.install)
			.then((result) => {
				display = result
			})
			.catch((error) => {
				displayError = String(error)
			})
	})
</script>

<div id="opt-{id ? md5(id) : 'top-level'}" use:observe={{ callback: (el, visible) => onVisible(el.id, visible) }}>
	{#if id}
		{@const option = findOption(manifest.options || [], id)! as ModOption & { type: "optionGroup" }}
		{#if display}
			<h2 class="text-2xl font-bold">{loc(option.name)}</h2>
			<p class="mb-2">
				{#await marked(loc(option.description || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
			</p>
			<div class="flex flex-wrap gap-2 mb-3">
				<sl-button variant="primary" size="small" onclick={() => void applyPreset(null)}>{m.ResetToDefaultsButton()}</sl-button>
				{#each option.presets || [] as preset}
					<sl-tooltip placement="bottom">
						<sl-button variant="primary" size="small" onclick={() => void applyPreset(preset.values)}>{loc(preset.name)}</sl-button>
						<div slot="content">
							{#if preset.image}
								{#await (async () => (preset.image ? commands.rsResolveModRelativePath(manifest.id, preset.image) : ""))() then imagePath}
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
			<div class="border-l-neutral-700 {id ? 'border-l pl-4' : ''}">
				<OptionsDisplay {manifest} {options} {reqCache} {clonedConfig} />
			</div>
		{:else if option.hiddenDescription}
			<h2 class="text-2xl font-bold text-neutral-400">{loc(option.name)}</h2>
			<p class="text-neutral-400">
				{#await marked(loc(option.hiddenDescription || ""), { gfm: true }) then x}{@html sanitiseInline(x)}{/await}
			</p>
		{/if}
		{#if displayError}
			<pre class="text-red-400">{displayError}</pre>
		{/if}
	{:else}
		<div class="border-l-neutral-700 {id ? 'border-l pl-4' : ''}">
			<OptionsDisplay {manifest} {options} {reqCache} {clonedConfig} />
		</div>
	{/if}
</div>

{#if display}
	<div class="border-l-neutral-700 {id ? 'border-l pl-4' : ''} option-groups">
		{#each options.filter((a) => a.type === "optionGroup").entries() as [ind, option]}
			<OptionsSection {manifest} id={option.id} {onVisible} {reqCache} {clonedConfig} {applyPreset} />
		{/each}
	</div>
{/if}

<style>
	:global(sl-checkbox::part(base)) {
		align-items: center;
	}

	:global(sl-radio::part(control)) {
		margin-top: 0.3rem;
	}

	:global(.option-groups > div:not(:first-child)) {
		@apply mt-4;
	}
</style>
