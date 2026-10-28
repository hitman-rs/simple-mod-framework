export type ModUpdate =
	| { type: "failed"; modName: string; error: string }
	| { type: "skipped"; modName: string }
	| { type: "upToDate"; modName: string }
	| { type: "developmentVersion"; version: string; modName: string }
	| { type: "autoUpdateAvailable"; oldVersion: string; newVersion: string; modId: string; modName: string; changelogIsURL: boolean; changelog: string; downloadURL: string }
	| { type: "nexusUpdateAvailable"; oldVersion: string; newVersion: string; modName: string; url: string }

export const data = $state({
	updates: {} as Record<string, ModUpdate>,
	modBeingChecked: null as string | null,
	nexusRateLimit: false,
	githubRateLimit: false
})

export default data
