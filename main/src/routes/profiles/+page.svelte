<script lang="ts">
	import { config } from "$lib/config.svelte"
	import { Masonry } from "svelte-bricks"
	import { v4 } from "uuid"
	import merge from "lodash.mergewith"
	import isEqual from "lodash.isequal"
	import { getModManifest, loc } from "$lib/mods.svelte"
	import * as m from "$lib/paraglide/messages"

	let nameModal: any = $state()
	let name = $state("")
</script>

<sl-dialog label={m.ProfileName()} bind:this={nameModal}>
	<sl-input label={m.EnterProfileName()} value={name} oninput={(evt) => (name = evt.target.value)}></sl-input>
	<br />
	<sl-button
		variant="primary"
		onclick={async () => {
			nameModal.hide()

			config.profiles[v4()] = JSON.parse(
				JSON.stringify({
					name,
					deployOrder: config.deployOrder,
					modOptions: Object.fromEntries(Object.entries(config.modOptions).filter((a) => config.deployOrder.includes(a[0])))
				})
			)

			name = ""
		}}>{m.ContinueButton()}</sl-button
	>
</sl-dialog>

{m.ProfilesDescription()}
<Masonry
	class="mt-4 h-[80vh] pr-2 overflow-y-auto"
	order="row-first"
	style="justify-content: start"
	items={[...Object.entries(config.profiles).map((a) => ({ id: a[0], profile: a[1] })), { id: null, profile: null }]}
	getId={(a) => a.id || "dummy"}
	gap={15}
>
	{#snippet children({ item: { id, profile } })}
		{#if profile}
			<sl-card class="w-full">
				<h2 class="text-xl font-bold" slot="header">{profile.name}</h2>
				<div>
					<ol class="list-decimal pl-8">
						{#each profile.deployOrder as mod}
							{#await getModManifest(mod) then manifest}
								<li>{loc(manifest.name)}</li>
							{:catch e}
								<li class="text-red-400">{e.message}</li>
							{/await}
						{/each}
					</ol>
				</div>
				<div slot="footer" class="flex flex-wrap gap-2">
					{#await Promise.all(profile.deployOrder.map((a) => getModManifest(a))) then}
						{#if !isEqual(config.deployOrder, profile.deployOrder) || !isEqual( config.modOptions, merge(JSON.parse(JSON.stringify(config.modOptions)), profile.modOptions, (orig, src) => {
									if (Array.isArray(orig)) {
										return src
									}
								}) )}
							<sl-button
								variant="primary"
								onclick={() => {
									config.deployOrder = JSON.parse(JSON.stringify(profile.deployOrder))

									merge(config.modOptions, JSON.parse(JSON.stringify(profile.modOptions)), (orig, src) => {
										if (Array.isArray(orig)) {
											return src
										}
									})
								}}>{m.SelectButton()}</sl-button
							>
						{:else}
							<sl-button variant="primary" disabled>{m.SelectedButton()}</sl-button>
						{/if}
					{:catch}
						<sl-button variant="primary" disabled>{m.SelectButton()}</sl-button>
						<sl-button
							variant="primary"
							onclick={async () => {
								for (const id of profile.deployOrder) {
									try {
										await getModManifest(id)
									} catch {
										profile.deployOrder = profile.deployOrder.filter((a) => a !== id)
										delete profile.modOptions[id]
									}
								}

								config.profiles[id] = profile

								if (profile.deployOrder.length === 0) {
									config.profiles = Object.fromEntries(Object.entries(config.profiles).filter((a) => a[0] !== id))
								}
							}}>{m.RemoveUnavailableModsButton()}</sl-button
						>
					{/await}
					<sl-button
						variant="danger"
						onclick={() => {
							config.profiles = Object.fromEntries(Object.entries(config.profiles).filter((a) => a[0] !== id))
						}}>{m.DeleteButton()}</sl-button
					>
				</div>
			</sl-card>
		{:else}
			<sl-card class="w-full">
				<h2 class="text-xl font-bold" slot="header">{m.CurrentSettings()}</h2>
				<div>{m.SaveProfileDesc({ mods: config.deployOrder.length })}</div>
				<div slot="footer" class="flex flex-wrap gap-2">
					<sl-button variant="primary" onclick={() => nameModal.show()}>{m.SaveAsProfileButton()}</sl-button>
				</div>
			</sl-card>
		{/if}
	{/snippet}
</Masonry>
