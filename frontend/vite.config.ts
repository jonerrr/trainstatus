import { sveltekit } from '@sveltejs/kit/vite';

import tailwindcss from '@tailwindcss/vite';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd());
	const allowedHosts = env.VITE_ALLOWED_HOSTS?.split(',');

	return {
		plugins: [tailwindcss(), sveltekit()],
		server: {
			proxy: {
				'/api': {
					target: 'http://localhost:3055',
					changeOrigin: true,
					rewrite: (path) => path.replace(/^\/api/, '')
				}
			},
			allowedHosts
			// : allowedHosts.length ? allowedHosts : undefined
		}
	};
});
