import { defineBrowserCommand } from '@vitest/browser-playwright';

export const dragMap = defineBrowserCommand(async ({ page, iframe }) => {
	const canvas = iframe.locator('[data-test-map] .maplibregl-canvas');
	const box = await canvas.boundingBox();
	if (!box) throw new Error('Map canvas is unavailable');
	const x = box.x + box.width * 0.8,
		y = box.y + box.height * 0.75;
	await page.mouse.move(x, y);
	await page.mouse.down();
	await page.mouse.move(x - 100, y + 30, { steps: 15 });
	await page.mouse.up();
});

declare module 'vitest/browser' {
	interface BrowserCommands {
		dragMap: () => Promise<void>;
	}
}
