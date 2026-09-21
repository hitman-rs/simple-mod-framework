module.exports = {
	content: ["./src/**/*.{html,svelte,js,ts}"],
	theme: {
		extend: {
			colors: {
				primary: {
					50: "#f3fbff",
					100: "#d6f0ff",
					200: "#b5e5ff",
					300: "#8fd8ff",
					400: "#59c4ff",
					500: "#06a7ff",
					600: "#008ad5",
					700: "#0071af",
					800: "#005e91",
					900: "#004367",
					950: "#002a40"
				}
			}
		}
	},
	plugins: [require("@tailwindcss/typography")]
}
