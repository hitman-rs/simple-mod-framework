<script lang="ts">
	import * as m from "$lib/paraglide/messages"

	let titleElem: HTMLElement | undefined = $state()
	let firstElem: HTMLElement | undefined = $state()
</script>

<div bind:this={titleElem}>
	<h1 class="text-6xl 2xl:text-7xl font-bold">
		{m.UnrecognisedGameVersion()}
		<sl-button
			pill
			variant="primary"
			size="large"
			onclick={() => {
				const utterance = new SpeechSynthesisUtterance(`${m.UnrecognisedGameVersion()}. ${m.UnrecognisedGameVersionUpdatedDesc()}`)
				utterance.rate = 0.8
				speechSynthesis.speak(utterance)
			}}
		>
			<sl-icon name="volume-up"></sl-icon>
			{m.ReadAloudButton()}
		</sl-button>
	</h1>
	<p class="mt-4 text-3xl 2xl:text-5xl font-bold">{m.UnrecognisedGameVersionReadReminder()}</p>
</div>
<p bind:this={firstElem} class="pt-4 2xl:pt-8 text-4xl 2xl:text-5xl font-bold text-green-300">{m.UnrecognisedGameVersionUpdatedDesc()}</p>
{#if titleElem && firstElem}
	{#each new Array(Math.floor(((visualViewport?.height || 0) - firstElem.offsetHeight - titleElem.offsetHeight - 100) / firstElem.offsetHeight)).keys() as idx}
		<p class="pt-4 2xl:pt-8 text-4xl 2xl:text-5xl font-bold {idx % 2 === 0 ? 'text-yellow-300' : 'text-green-300'}">{m.UnrecognisedGameVersionUpdatedDesc()}</p>
	{/each}
{/if}
