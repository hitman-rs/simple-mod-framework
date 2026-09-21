<script lang="ts">
	import { commands } from "$lib/bindings"
	import type { StringOptionValidation, StringOptionValidation_Deserialize, ValidationResult } from "$lib/bindings"

	let {
		value = $bindable(),
		label,
		disabled = false,
		validation
	}: {
		label?: string
		value: string
		disabled?: boolean
		validation: StringOptionValidation
	} = $props()

	let validationResult: ValidationResult = $state({ result: "pass" })
</script>

<sl-input
	{label}
	type="text"
	{value}
	{disabled}
	pattern={validation.pattern || undefined}
	onsl-change={async (evt) => {
		if (typeof evt.target.value === "string") {
			validationResult = await commands.rsValidateStringOption(validation as StringOptionValidation_Deserialize, evt.target.value)

			if (validationResult.result === "pass" && value !== evt.target.value) {
				value = evt.target.value
			}
		}
	}}
></sl-input>
{#if validationResult.result === "fail"}
	<div class="text-red-300 mt-2 text-sm">{validationResult.message}</div>
{/if}
