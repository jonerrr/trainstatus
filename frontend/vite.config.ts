import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

import tailwindcss from '@tailwindcss/vite';
import { playwright } from '@vitest/browser-playwright';
import { loadEnv } from 'vite';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd());
	const allowedHosts = env.VITE_ALLOWED_HOSTS?.split(',');

	return {
		// See: https://github.com/MIERUNE/svelte-maplibre-gl/issues/206
		optimizeDeps: {
			exclude: ['maplibre-gl']
		},
		plugins: [
			tailwindcss(),
			sveltekit({
				// Consult https://kit.svelte.dev/docs/integrations#preprocessors
				// for more information about preprocessors
				preprocess: vitePreprocess(),
				compilerOptions: { experimental: { async: true } },
				inspector: true,
				// adapter-auto only supports some environments, see https://kit.svelte.dev/docs/adapter-auto for a list.
				// If your environment is not supported, or you settled on a specific environment, switch out the adapter.
				// See https://kit.svelte.dev/docs/adapters for more information about adapters.
				adapter: adapter()
			})
		],
		server: {
			proxy: {
				// backend
				'/api': {
					target: 'http://localhost:3055',
					changeOrigin: true
				},
				// martin server
				'/martin': {
					target: 'http://localhost:3000',
					changeOrigin: true,
					xfwd: true
				}
			},
			allowedHosts
		},
		test: {
			expect: { requireAssertions: true },
			projects: [
				{
					extends: './vite.config.ts',
					test: {
						name: 'client',
						browser: {
							enabled: true,
							provider: playwright(),
							instances: [{ browser: 'chromium', headless: true }]
						},
						include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
						exclude: ['src/lib/server/**']
					}
				},

				{
					extends: './vite.config.ts',
					test: {
						name: 'server',
						environment: 'node',
						include: ['src/**/*.{test,spec}.{js,ts}'],
						exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
					}
				}
			]
		}
	};
});
