<script lang="ts">
	import { goto } from "$app/navigation"
	import { page } from "$app/state"
	import { resolve } from "@tauri-apps/api/path"
	import { onMount } from "svelte"
	import { download } from "@tauri-apps/plugin-upload"
	import * as m from "$lib/paraglide/messages"

	let downloading = $state(false)
	let error: unknown | null = $state(null)
	let downloadProgress = $state(0)

	onMount(async () => {
		try {
			downloading = true

			await download(page.url.searchParams.get("url")!, await resolve("tempArchive"), ({ progressTotal, total }) => (downloadProgress = (progressTotal / total) * 100))

			goto(`/mods?addAndDeleteFile=${await resolve("tempArchive")}`)
		} catch (e) {
			downloading = false
			error = e
			return
		}
	})
</script>

<h1 class="text-4xl 2xl:text-5xl font-bold">{m.Downloading()}</h1>
{#if downloading}
	{m.ModBeingDownloaded()}
	<sl-progress-bar class="mt-2" value={downloadProgress}></sl-progress-bar>
{:else if error}
	{m.CouldntDownloadMod()}
	<pre class="mt-2"><code>{error}</code></pre>
{/if}
