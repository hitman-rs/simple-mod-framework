const fs = require("fs")

const quickentity21 = require("./quickentity21")
const quickentity20 = require("./quickentity20")
const quickentity1136 = require("./quickentity1136")
const LosslessJSON = require("lossless-json")

const targetPath = process.argv[3]

;(async () => {
	switch (process.argv[2]) {
		case "convert":
			switch (process.argv[8]) {
				case "2.1":
					await quickentity21.convert(
						"HM3",
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7],
						targetPath
					)
					break
				case "2.0":
					await quickentity20.convert(
						"HM3",
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7],
						targetPath
					)
					break
				case "1.136":
					await quickentity1136.convert(
						"HM3",
						"ids",
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7],
						targetPath
					)
					break
			}
			break

		case "generate":
			switch (+LosslessJSON.parse(fs.readFileSync(targetPath, "utf8")).quickEntityVersion.value) {
				case 2.1:
					await quickentity21.generate(
						"HM3",
						targetPath,
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7]
					)
					break
				case 2.0:
					await quickentity20.generate(
						"HM3",
						targetPath,
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7]
					)
					break
				case 1.136:
					await quickentity1136.generate(
						"HM3",
						targetPath,
						process.argv[4],
						process.argv[5],
						process.argv[6],
						process.argv[7]
					)
					break
			}
			break

		case "applyPatch":
			switch (+LosslessJSON.parse(fs.readFileSync(process.argv[5], "utf8")).patchVersion.value) {
				case 4:
					await quickentity21.applyPatchJSON(process.argv[4], process.argv[5], targetPath)
					break
				case 3:
					await quickentity20.applyPatchJSON(process.argv[4], process.argv[5], targetPath)
					break
				case 2:
				case 1:
					await quickentity1136.applyPatchJSON(process.argv[4], process.argv[5], targetPath)
					break
			}
			break
	}
})()
