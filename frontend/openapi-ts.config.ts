import { defineConfig } from '@hey-api/openapi-ts';

export default defineConfig({
	input: 'http://localhost:5173/api/openapi.json',
	output: 'src/lib/client',
	plugins: [
		'@hey-api/typescript',
		// TODO: find a way to not include the transformers.gen.ts but still have the type be date
		{ name: '@hey-api/transformers', dates: true }
	]
});
