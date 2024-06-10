/** @type {import('tailwindcss').Config} */
export default {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	theme: {
		fontFamily: {
			sans: ['Inter', 'sans-serif']
		},
		extend: {}
	},
	plugins: [require('@tailwindcss/forms')]
};

// primary background
// secondary background
// primary text
// secondary text
// tertiary text
// primary border
