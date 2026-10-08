import adapter from '@sveltejs/adapter-node';
import { enhancedImages } from '@sveltejs/enhanced-img';
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
			enhancedImages(),
			tailwindcss(),
			sveltekit({
				// Consult https://kit.svelte.dev/docs/integrations#preprocessors
				// for more information about preprocessors
				preprocess: vitePreprocess(),
				csp: {
					mode: 'auto',
					directives: {
						'default-src': ['self'],
						'base-uri': ['self'],
						'connect-src': ['self', 'data:', 'https://cloudflareinsights.com'],
						'font-src': ['self'],
						'frame-ancestors': ['none'],
						'form-action': ['self'],
						'img-src': ['self', 'data:', 'blob:'],
						'object-src': ['none'],
						'script-src': ['self', 'https://static.cloudflareinsights.com'],
						'style-src': ['self'],
						'style-src-attr': ['unsafe-inline'],
						'worker-src': ['self', 'blob:'],
						'upgrade-insecure-requests': true
					}
				},
				compilerOptions: {
					runes: ({ filename }) =>
						filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
					experimental: { async: true }
				},
				inspector: true,
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
		preview: {
			proxy: {
				'/api': {
					target: process.env.API_ORIGIN ?? 'http://localhost:3055',
					changeOrigin: true
				},
				'/martin': {
					target: 'http://127.0.0.1:3000',
					changeOrigin: true,
					xfwd: true
				}
			}
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
						include: ['src/**/*.svelte.test.ts'],
						exclude: ['src/lib/server/**'],
						setupFiles: ['src/lib/test/client-setup.ts']
					}
				},
				{
					extends: './vite.config.ts',
					test: {
						name: 'server',
						environment: 'node',
						include: ['src/**/*.test.ts'],
						exclude: ['src/**/*.svelte.test.ts', 'tests/integration/**']
					}
				},
				{
					extends: './vite.config.ts',
					test: {
						name: 'integration',
						environment: 'node',
						include: ['tests/integration/**/*.test.ts']
					}
				}
			]
		}
	};
});
