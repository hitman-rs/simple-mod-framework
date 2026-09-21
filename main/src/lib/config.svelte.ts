import { readTextFile, exists, writeTextFile, watch } from "@tauri-apps/plugin-fs"
import { join } from "@tauri-apps/api/path"
import { getAllMods, getModManifest } from "./mods.svelte"
import { commands, type ModOption, type Config } from "./bindings"
import { useDebounce } from "runed"
import isEqual from "lodash.isequal"

export const gameInstalls = commands.rsDetectGameInstalls()

async function getConfig(): Promise<Config> {
	const parsed: Config = JSON.parse(await readTextFile("config.json"))
	await validateConfig(parsed)
	return parsed
}

/**
 * @param seenValuedOptions Keeps track of all option IDs that have been seen and can hold a value.
 */
function recursivelyValidateOptions(config: Config, mod: string, options: ModOption[], seenValuedOptions: string[]) {
	config.modOptions[mod] ??= {}

	for (const option of options) {
		switch (option.type) {
			case "boolean":
				seenValuedOptions.push(option.id)
				if (config.modOptions[mod][option.id]?.type !== "boolean") {
					config.modOptions[mod][option.id] = { type: "boolean", value: option.defaultValue }
				}
				break
			case "selection":
				seenValuedOptions.push(option.id)
				if (config.modOptions[mod][option.id]?.type !== "selection" || !option.options.map((a) => a.id).includes(config.modOptions[mod][option.id]!.value as string)) {
					config.modOptions[mod][option.id] = { type: "selection", value: option.defaultValue }
				}
				break
			case "number":
				seenValuedOptions.push(option.id)
				if (config.modOptions[mod][option.id]?.type !== "number") {
					config.modOptions[mod][option.id] = { type: "number", value: option.defaultValue }
				}
				break
			case "color":
				seenValuedOptions.push(option.id)
				if (config.modOptions[mod][option.id]?.type !== "color") {
					config.modOptions[mod][option.id] = { type: "color", value: option.defaultValue }
				}
				break
			case "string":
				seenValuedOptions.push(option.id)
				if (config.modOptions[mod][option.id]?.type !== "string") {
					config.modOptions[mod][option.id] = { type: "string", value: option.defaultValue }
				}
				break
			case "optionGroup":
				recursivelyValidateOptions(config, mod, option.options, seenValuedOptions)
				break
			case "conditional":
				break
			default:
				throw option satisfies never
		}
	}
}

export interface GUIConfig {
	firstTimeUser: boolean
	knownMods: string[]
	knownModOptions: Record<string, string[]>
}

export function gui(config: Config) {
	return config.gui as unknown as GUIConfig
}

export async function validateConfig(config: Config) {
	const allMods = await getAllMods()

	if (Object.keys(config.gui).length === 0) {
		config.gui = {
			firstTimeUser: true,
			knownMods: [],
			knownModOptions: {}
		} satisfies GUIConfig as unknown as Config["gui"]
	}

	for (const mod of config.deployOrder) {
		if (!allMods.includes(mod)) {
			config.deployOrder.splice(config.deployOrder.indexOf(mod), 1)
		}
	}

	for (const [idx, knownMod] of gui(config).knownMods.entries().toArray().reverse()) {
		if (gui(config).knownMods.indexOf(knownMod) !== idx) {
			gui(config).knownMods.splice(idx, 1)
		}
	}

	await Promise.all(
		allMods.map(async (mod) => {
			const manifest = await getModManifest(mod)

			if (manifest.options) {
				const seenOptions: string[] = []
				recursivelyValidateOptions(config, mod, manifest.options, seenOptions)
				for (const id of Object.keys(config.modOptions[mod]!).filter((id) => !seenOptions.includes(id))) {
					delete config.modOptions[mod]![id]
				}
			} else {
				if (config.modOptions[mod] && Object.keys(config.modOptions[mod]).length > 0) {
					config.modOptions[mod] = {}
				}
			}
		})
	)

	if (config.gamePath && !(await exists(await join(config.gamePath, "thumbs.dat")))) {
		config.gamePath = null
	}
}

export const config = $state({
	PLACEHOLDER: true,

	gamePath: null,
	developerMode: false,
	deployOrder: [],
	modOptions: {},
	profiles: {},
	onlineServices: false,
	skipIntro: true,
	useAlternativeOutputDirectory: null,
	writeDeploySummary: true,
	autoDisableDynres: true,
	uiLocale: "en",
	bootScene: null,

	gui: {}
} as Config)

let lastSaved = {} as Config

export const saveConfig = useDebounce(async () => {
	if (config["PLACEHOLDER"]) return

	await validateConfig(config)

	if (!isEqual(lastSaved, $state.snapshot(config))) {
		lastSaved = $state.snapshot(config)
		await writeTextFile("config.json", JSON.stringify(config, undefined, "\t"))
	}
}, 250)

export async function updateConfig(): Promise<Config> {
	const parsed = await getConfig()
	if (isEqual(lastSaved, parsed)) return config
	Object.assign(config, parsed)
	for (const key of Object.keys(config).filter((a) => !Object.hasOwn(parsed, a))) {
		delete config[key]
	}
	return config
}

void updateConfig()

void watch(
	"config.json",
	(e) => {
		if (typeof e.type === "string" || Object.hasOwn(e.type, "create") || Object.hasOwn(e.type, "modify") || Object.hasOwn(e.type, "remove")) {
			void updateConfig()
		}
	},
	{ delayMs: 100 }
)
