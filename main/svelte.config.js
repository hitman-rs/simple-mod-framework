import adapter from "@sveltejs/adapter-static"
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte"

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),

	kit: {
		adapter: adapter()
	},

	compilerOptions: {
		warningFilter: (warning) => {
			if (warning.code === "a11y_click_events_have_key_events" || warning.code === "a11y_no_static_element_interactions") {
				// Svelte complains about custom elements with (correct) onclick handlers
				return false
			}

			return true
		}
	}
}

export default config
