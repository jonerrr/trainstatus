import { test as base, expect } from '@playwright/test';

export const test = base.extend({
	page: async ({ page }, use) => {
		await page.context().clearCookies();
		await page.context().addInitScript(() => {
			const flag = 'e2e-storage-cleared';
			if (sessionStorage.getItem(flag)) return;
			localStorage.clear();
			sessionStorage.setItem(flag, '1');
		});
		await page.goto('/');
		await expect(page.getByRole('navigation')).toBeVisible();
		await use(page);
	}
});
export { expect };
