import devtoolsJson from "vite-plugin-devtools-json"
import { defineConfig } from "vite"
import { sveltekit } from "@sveltejs/kit/vite"
import { paraglideVitePlugin } from "@inlang/paraglide-js"

export default defineConfig({
	plugins: [
		sveltekit(),
		paraglideVitePlugin({
			project: "../project.inlang",
			outdir: "./src/lib/paraglide"
		}),
		devtoolsJson()
	],
	build: { target: "es2022" }
})
