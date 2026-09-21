<script lang="ts">
	import { commands } from "$lib/bindings"
	import type { NumberOptionValidation, NumberOptionValidation_Deserialize, ValidationResult } from "$lib/bindings"

	let {
		value = $bindable(),
		label,
		disabled = false,
		validation
	}: {
		label?: string
		value: number
		disabled?: boolean
		validation: NumberOptionValidation
	} = $props()

	let validationResult: ValidationResult = $state({ result: "pass" })
</script>

<sl-input
	{label}
	type="number"
	{value}
	{disabled}
	min={validation.minimum || undefined}
	max={validation.maximum || undefined}
	step={validation.integer ? 1 : "any"}
	onsl-change={async (evt) => {
		if (!Number.isNaN(Number(evt.target.value))) {
			validationResult = await commands.rsValidateNumberOption(validation as NumberOptionValidation_Deserialize, Number(evt.target.value))

			if (validationResult.result === "pass" && value !== Number(evt.target.value)) {
				value = Number(evt.target.value)
			}
		}
	}}
></sl-input>
{#if validationResult.result === "fail"}
	<div class="text-red-300 mt-2 text-sm">{validationResult.message}</div>
{/if}
