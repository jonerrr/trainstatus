import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: './e2e',
	testMatch: '**/*.e2e.ts',
	fullyParallel: true,
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	workers: process.env.CI ? 1 : undefined,
	use: {
		baseURL: 'http://127.0.0.1:4173',
		trace: 'retain-on-failure',
		// Keep each test independent of cached responses from the app's service worker.
		serviceWorkers: 'block'
	},
	projects: [
		{ name: 'chromium', use: { ...devices['Desktop Chrome'] } },
		{ name: 'map-firefox', testMatch: '**/map.e2e.ts', use: { ...devices['Desktop Firefox'] } },
		{ name: 'map-touch', testMatch: '**/map.e2e.ts', use: { ...devices['Pixel 7'] } }
	],
	webServer: {
		command: 'pnpm build && pnpm preview --host 127.0.0.1 --port 4173 --strictPort',
		url: 'http://127.0.0.1:4173',
		env: { API_ORIGIN: process.env.API_ORIGIN ?? 'http://127.0.0.1:3055' },
		timeout: 120_000
	}
});
