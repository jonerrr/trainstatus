import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';

import { dragMap } from './src/lib/test/map-commands.ts';

export default defineConfig({
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
						commands: { dragMap },
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
});
