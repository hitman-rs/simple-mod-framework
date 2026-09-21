<script lang="ts">
	import type { Config, Manifest, ModOption } from "$lib/bindings"
	import { MasonryGrid } from "@egjs/grid/dist/grid"
	import { useDebounce, watch } from "runed"
	import { untrack } from "svelte"
	import Option from "./Option.svelte"

	interface Props {
		manifest: Manifest
		options: ModOption[]
		reqCache: {
			install: { version: string; platform: string } | null
			enabledMods: Record<string, string>
		}
		clonedConfig: Config
	}

	let { manifest, options, reqCache, clonedConfig }: Props = $props()

	let grid: MasonryGrid | null = $state(null)
	let width = $state(0)
	let cols = $derived(Math.floor(width / 400))
	let colSize = $derived(Math.round((width - (cols - 1) * 16) / cols))

	const rerender = useDebounce(() => {
		if (grid) {
			grid.column = cols
			grid.columnSize = colSize
			requestAnimationFrame(() => {
				grid.renderItems()
			})
		}
	}, 50)

	$effect(() => {
		if (grid && options.filter((a) => !["optionGroup", "conditional"].includes(a.type)).length) {
			untrack(() => void rerender())
		}
	})

	watch(
		() => colSize,
		() => void rerender()
	)
</script>

<div
	bind:clientWidth={width}
	{@attach (el) => {
		if (!grid) {
			grid = new MasonryGrid(el, {
				gap: 16,
				column: cols,
				columnSize: colSize,
				useResizeObserver: true,
				observeChildren: true
			})
		}
	}}
>
	{#each options.filter((a) => !["optionGroup", "conditional"].includes(a.type)) as option (option.id)}
		<Option {option} {manifest} {colSize} {reqCache} {clonedConfig} />
	{/each}
</div>
