import { defineConfig } from '@hey-api/openapi-ts';

export default defineConfig({
	input: 'http://localhost:5173/api/openapi.json',
	output: { path: 'src/lib/client', postProcess: ['prettier'] },
	plugins: ['@hey-api/typescript']
});
