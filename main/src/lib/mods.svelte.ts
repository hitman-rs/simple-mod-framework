import type { Manifest, ModInfo, ModOption, UIText } from "./bindings"
import { readDir, readTextFile, watch, exists } from "@tauri-apps/plugin-fs"
import { join, sep } from "@tauri-apps/api/path"
import { commands, type Config } from "./bindings"
import { gameInstalls } from "./config.svelte"
import { getLocale, type locales } from "./paraglide/runtime"
import { fetch } from "@tauri-apps/plugin-http"
import isEqual from "lodash.isequal"

export const allMods: string[] = $state([])
let pendingModListUpdate: Promise<void> | null = null
const modManifests: Record<string, Manifest> = $state({})

async function updateModList() {
	const newMods = (await readDir("Mods")).map((a) => a.name).filter((a) => a && !a.startsWith(".") && !["DO NOT TOUCH THIS FOLDER", "desktop.ini"].includes(a)) as string[]

	for (const mod of (allMods || []).filter((a) => !newMods.includes(a))) {
		delete modManifests[mod]
		allMods.splice(allMods.indexOf(mod), 1)
	}

	for (const mod of newMods.filter((a) => !(allMods || []).includes(a))) {
		allMods.push(mod)
	}
}

pendingModListUpdate = updateModList().then(() => {
	pendingModListUpdate = null
})

void readDir("Mods").then((mods) => {
	void watch(
		["Mods", ...mods.filter((a) => a.isDirectory).map((a) => `Mods${sep()}${a.name}`), ...mods.filter((a) => a.isDirectory).map((a) => `Mods${sep()}${a.name}${sep()}manifest.json`)],
		(e) => {
			if (typeof e.type === "string" || Object.hasOwn(e.type, "create") || Object.hasOwn(e.type, "modify") || Object.hasOwn(e.type, "remove")) {
				for (const path of e.paths) {
					if (path.endsWith("manifest.json")) {
						const modId = path.match(/[\\/]Mods[\\/]([^\\/]+)[\\/]manifest\.json/)?.[1]
						if (modId && modManifests[modId]) {
							void exists(path).then(async (exists) => {
								if (exists) {
									try {
										if (modManifests[modId]) {
											modManifests[modId] = JSON.parse(await readTextFile(path))
										}
									} catch (e) {
										console.error(`Couldn't read manifest.json for ${modId}`, e)
									}
								}
							})
						}
					} else {
						if (path.match(/[\\/]Mods[\\/][^\\/]+[\\/]?/)) {
							pendingModListUpdate = updateModList().then(() => {
								pendingModListUpdate = null
							})
						}
					}
				}
			}
		},
		{ delayMs: 800 }
	)
})

/** Get the list of all installed mods, ensuring it is up-to-date. */
export async function getAllMods() {
	if (pendingModListUpdate) await pendingModListUpdate
	return allMods
}

export async function getModManifest(id: string): Promise<Manifest> {
	if (!modManifests[id]) {
		try {
			modManifests[id] = JSON.parse(await readTextFile(await join("Mods", id, "manifest.json")))
			if (modManifests[id].id !== id) {
				throw new Error(`Mod ID ${modManifests[id].id} in manifest.json does not match expected ID ${id}`)
			}
		} catch (e) {
			if (!(await exists(await join("Mods", id)))) {
				throw new Error(`${id} is not installed`)
			}

			if (!(await exists(await join("Mods", id, "manifest.json")))) {
				throw new Error(`${id} was not installed correctly, remove it and use the Add a Mod button`)
			}

			throw new Error(`Couldn't read manifest.json for ${id}: ${e}`)
		}
	}

	return modManifests[id]
}

export function reformatModID(id: string) {
	return id
		.replaceAll(/(?:_|-)([a-z])/g, (_, x) => x.toUpperCase())
		.replaceAll(/_|-/g, "")
		.replaceAll(/\.([a-z])/g, (_, x) => `.${x.toUpperCase()}`)
}

export const chunkPartitions = {
	chunk0: "super",
	chunk1: "base",
	chunk2: "season3",
	chunk3: "ancestral",
	chunk4: "edgy",
	chunk5: "elegant",
	chunk6: "wet",
	chunk7: "trapped",
	chunk8: "legacy",
	chunk9: "season2",
	chunk10: "opulent",
	chunk11: "caged",
	chunk12: "greedy",
	chunk13: "salty",
	chunk14: "hawk",
	chunk15: "theark",
	chunk16: "skunk",
	chunk17: "mongoose",
	chunk18: "colombia",
	chunk19: "miami",
	chunk20: "sheep",
	chunk21: "season1",
	chunk22: "hokkaido",
	chunk23: "colorado",
	chunk24: "bangkok",
	chunk25: "marrakesh",
	chunk26: "coastaltown",
	chunk27: "paris",
	chunk28: "dugong",
	chunk29: "snug"
}

export async function getH3GamePath(config: Config) {
	const installs = await gameInstalls

	if (config.gamePath && installs.find((a) => a.path === config.gamePath)?.version === "h3") {
		return config.gamePath
	} else {
		const found = installs.find((a) => a.version === "h3")?.path
		if (!found) throw new Error("Couldn't find any version of HITMAN 3 to use")
		return found
	}
}

/** Localise a manifest UIText to the currently active UI locale for display. */
export function loc(text: UIText): string {
	if (typeof text === "string") {
		return text
	} else {
		const localised: Partial<Record<(typeof locales)[number], string | null>> = {
			en: text.english,
			fr: text.french,
			it: text.italian,
			de: text.german,
			es: text.spanish,
			"es-MX": text.spanish_mexico || text.spanish, // Fallback es-MX to es
			"pt-BR": text.portuguese_brazil,
			tr: text.turkish,
			pl: text.polish,
			ru: text.russian,
			"zh-Hans": text.chinese_simplified,
			"zh-Hant": text.chinese_traditional,
			ja: text.japanese,
			ko: text.korean
		}

		const lang = getLocale()
		if (localised[lang]) return localised[lang]

		// Otherwise return first specified in game order
		for (const lang of ["en", "fr", "it", "de", "es", "ru", "es-MX", "pt-BR", "pl", "zh-Hans", "ja", "zh-Hant", "ko", "tr"] as const) {
			if (localised[lang]) return localised[lang]
		}

		throw new Error("No localisation specified")
	}
}

let v2Mods: Record<string, ModInfo> | null = null

export async function getV2ModInfo() {
	if (v2Mods !== null) {
		return v2Mods
	} else {
		try {
			v2Mods = await (await fetch("https://hitman-resources.netlify.app/smf-api/v2-mods")).json()
			return v2Mods!
		} catch (e) {
			throw new Error(`Failed to fetch mod info, check your internet connection: ${e}`)
		}
	}
}

const conditionCachedConfig = {
	config: {} as Config,
	enabledMods: {} as Record<string, string>,
	install: null as { version: string; platform: string } | null
}
let conditionCache: Record<string, Promise<boolean>> = {}

export async function evaluateCondition(mod: string, condition: string, enabledMods: Record<string, string>, install: { version: string; platform: string } | null, config: Config) {
	if (!isEqual(config, conditionCachedConfig.config) || !isEqual(enabledMods, conditionCachedConfig.enabledMods) || !isEqual(install, conditionCachedConfig.install)) {
		conditionCachedConfig.config = JSON.parse(JSON.stringify(config))
		conditionCachedConfig.enabledMods = JSON.parse(JSON.stringify(enabledMods))
		conditionCachedConfig.install = install
		conditionCache = {}
	}

	const cached = conditionCache[`${mod}:${condition}`]
	if (cached !== undefined) {
		return await cached
	}

	const evaluated = commands.rsEvaluateCondition(mod, condition, enabledMods, install ? `${install.version}-${install.platform}` : "h3-epic", config)
	conditionCache[`${mod}:${condition}`] = evaluated
	return await evaluated
}

export function getAllOptions(opts: ModOption[]) {
	return opts.flatMap((opt) => (opt.type === "optionGroup" ? [opt.id, ...getAllOptions(opt.options)] : [opt.id]))
}
