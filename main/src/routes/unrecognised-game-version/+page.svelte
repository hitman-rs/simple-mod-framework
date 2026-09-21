<script lang="ts">
	import * as m from "$lib/paraglide/messages"

	let titleElem: HTMLElement | undefined = $state()
	let firstElem: HTMLElement | undefined = $state()
</script>

<div class="lg:grid grid-cols-2 gap-8 mb-4">
	<div>
		<div bind:this={titleElem}>
			<h1 class="text-6xl 2xl:text-7xl font-bold">
				{m.UnrecognisedGameVersion()}
				<sl-button
					pill
					variant="primary"
					size="large"
					onclick={() => {
						const utterance = new SpeechSynthesisUtterance(`${m.UnrecognisedGameVersion()}. ${m.UnrecognisedGameVersionDesc()} ${m.UnrecognisedGameVersionDesc2()}`)
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
		<p bind:this={firstElem} class="pt-4 2xl:pt-8 text-3xl 2xl:text-5xl font-bold text-yellow-300">{m.UnrecognisedGameVersionDesc()}</p>
		{#if titleElem && firstElem}
			{#each new Array(Math.floor(((visualViewport?.height || 0) - firstElem.offsetHeight - titleElem.offsetHeight - 100) / firstElem.offsetHeight)).keys() as idx}
				<p class="pt-4 2xl:pt-8 text-3xl 2xl:text-5xl font-bold {idx % 2 === 0 ? 'text-cyan-300' : 'text-yellow-300'}"
					>{idx % 2 === 0 ? m.UnrecognisedGameVersionDesc2() : m.UnrecognisedGameVersionDesc()}</p
				>
			{/each}
		{/if}
	</div>
	<div>
		<p class="text-3xl 2xl:text-5xl font-bold text-yellow-300">{m.UnrecognisedGameVersionDesc()}</p>
		{#if titleElem && firstElem}
			{#each new Array(Math.floor(((visualViewport?.height || 0) - firstElem.offsetHeight * 2 - titleElem.offsetHeight - 100) / firstElem.offsetHeight)).keys() as idx}
				<p class="pt-4 2xl:pt-8 text-3xl 2xl:text-5xl font-bold {idx % 2 === 0 ? 'text-cyan-300' : 'text-yellow-300'}"
					>{idx % 2 === 0 ? m.UnrecognisedGameVersionDesc2() : m.UnrecognisedGameVersionDesc()}</p
				>
			{/each}
		{/if}
		<h2 class="mt-4 2xl:mt-8 text-5xl 2xl:text-6xl font-bold">{m.UnrecognisedGameVersionUpdateQuestion()}</h2>
		<div class="mt-4 flex flex-wrap gap-x-4 gap-y-2">
			<sl-button size="large" variant="success" href="/unrecognised-game-version/updated">{m.YesButton()}</sl-button>
			<sl-button size="large" variant="danger" href="/unrecognised-game-version/not-updated">{m.NoButton()}</sl-button>
		</div>
	</div>
</div>
