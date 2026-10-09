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

/** Localise a manifest UIText to the specified, or otherwise currently active, UI locale for display. */
export function loc(text: UIText, locale?: string): string {
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

		const lang = locale || getLocale()
		if (typeof localised[lang] !== "undefined" && localised[lang] !== null) return localised[lang]

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

export const localeSmf = {
	en: "english",
	fr: "french",
	it: "italian",
	de: "german",
	es: "spanish",
	"es-MX": "spanishMexico",
	"pt-BR": "portugueseBrazil",
	tr: "turkish",
	pl: "polish",
	ru: "russian",
	"zh-Hans": "chineseSimplified",
	"zh-Hant": "chineseTraditional",
	ja: "japanese",
	ko: "korean"
} as const

export function linesToTranslate(manifest: Manifest, locale: (typeof locales)[number]): string[] {
	const lines: string[] = []
	const consider = (path: string, line: UIText) => {
		if (typeof line === "string") {
			if (locale !== "en") {
				lines.push(path)
			}
		} else {
			if (typeof line[localeSmf[locale]] === "undefined") {
				lines.push(path)
			}
		}
	}

	consider("/name", manifest.name)
	consider("/description", manifest.description)

	manifest.conditions?.requiredConditions?.forEach((cond, i) => {
		consider(`/conditions/requiredConditions/${i}/explanation`, cond.explanation)
	})

	manifest.conditions?.incompatibleConditions?.forEach((cond, i) => {
		consider(`/conditions/incompatibleConditions/${i}/explanation`, cond.explanation)
	})

	manifest.presets?.forEach((preset, i) => {
		consider(`/presets/${i}/name`, preset.name)
		if (preset.description) consider(`/presets/${i}/description`, preset.description)
	})

	for (const [key, loc] of Object.entries(manifest.data?.localisation || {})) {
		consider(`/data/localisation/${key}`, loc)
	}

	const recurseOptions = (opts: ModOption[], path: string) => {
		opts.forEach((opt, i) => {
			if ("name" in opt) consider(`${path}/${i}/name`, opt.name)
			if ("description" in opt && opt.description) consider(`${path}/${i}/description`, opt.description)
			if ("conditions" in opt) {
				opt.conditions?.requiredConditions?.forEach((cond, j) => {
					consider(`${path}/${i}/conditions/requiredConditions/${j}/explanation`, cond.explanation)
				})

				opt.conditions?.incompatibleConditions?.forEach((cond, j) => {
					consider(`${path}/${i}/conditions/incompatibleConditions/${j}/explanation`, cond.explanation)
				})
			}
			if ("data" in opt) {
				for (const [key, loc] of Object.entries(opt.data?.localisation || {})) {
					consider(`${path}/${i}/data/localisation/${key}`, loc)
				}
			}
			if (opt.type === "selection") {
				for (const [j, subOption] of opt.options.entries()) {
					consider(`${path}/${i}/options/${j}/name`, subOption.name)
					if (subOption.description) consider(`${path}/${i}/options/${j}/description`, subOption.description)
					if ("conditions" in subOption) {
						subOption.conditions?.requiredConditions?.forEach((cond, k) => {
							consider(`${path}/${i}/options/${j}/conditions/requiredConditions/${k}/explanation`, cond.explanation)
						})
						subOption.conditions?.incompatibleConditions?.forEach((cond, k) => {
							consider(`${path}/${i}/options/${j}/conditions/incompatibleConditions/${k}/explanation`, cond.explanation)
						})
					}
					if ("data" in subOption) {
						for (const [key, loc] of Object.entries(subOption.data?.localisation || {})) {
							consider(`${path}/${i}/options/${j}/data/localisation/${key}`, loc)
						}
					}
				}
			} else if (opt.type === "optionGroup") {
				if (opt.hiddenDescription) consider(`${path}/${i}/hiddenDescription`, opt.hiddenDescription)
				opt.presets?.forEach((preset, j) => {
					consider(`${path}/${i}/presets/${j}/name`, preset.name)
					if (preset.description) consider(`${path}/${i}/presets/${j}/description`, preset.description)
				})
				recurseOptions(opt.options, `${path}/${i}/options`)
			}
		})
	}

	if (manifest.options) {
		recurseOptions(manifest.options, "/options")
	}

	return lines
}
