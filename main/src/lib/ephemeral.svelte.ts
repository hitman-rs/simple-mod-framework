import { appLocalDataDir } from "@tauri-apps/api/path"
import { BaseDirectory, exists, mkdir, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs"
import { useDebounce } from "runed"

export const versionCache = $state({
	time: 0,
	versions: {} as Record<string, string>
})

void (async () => {
	if (await exists("version_cache.json", { baseDir: BaseDirectory.AppLocalData })) {
		try {
			const cache = JSON.parse(await readTextFile("version_cache.json", { baseDir: BaseDirectory.AppLocalData }))
			versionCache.time = cache.time || 0
			versionCache.versions = cache.versions || {}

			if (Date.now() - versionCache.time > 1000 * 60 * 60 * 1) {
				versionCache.time = Date.now()
				versionCache.versions = {}
			}
		} catch {}
	} else {
		versionCache.time = Date.now()
	}
})()

export const saveVersionCache = useDebounce(async () => {
	await mkdir(await appLocalDataDir(), { recursive: true })
	await writeTextFile("version_cache.json", JSON.stringify(versionCache, undefined, "\t"), {
		baseDir: BaseDirectory.AppLocalData
	})
}, 1000)
