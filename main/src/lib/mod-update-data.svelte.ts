export type ModUpdate =
	| { type: "failed"; modName: string; error: string }
	| { type: "upToDate"; modName: string }
	| { type: "developmentVersion"; version: string; modName: string }
	| { type: "autoUpdateAvailable"; oldVersion: string; newVersion: string; modId: string; modName: string; changelogIsURL: boolean; changelog: string; downloadURL: string }
	| { type: "nexusUpdateAvailable"; oldVersion: string; newVersion: string; modName: string; url: string }

export const data = $state({
	updates: [] as ModUpdate[],
	checkingFinished: false,
	modBeingChecked: "mods",
	skipped: 0
})

export default data
