import { expect, test } from './fixtures';

const pages = [
	['Home', '/'],
	['Alerts', '/alerts'],
	['Stops', '/stops'],
	['Charts', '/charts'],
	['Settings', '/settings']
] as const;

const labels = ['Home', 'Alerts', 'Stops', 'Charts', 'Map', 'Settings'];

for (const [name, path] of pages) {
	test(`${name} marks only that navbar link as the current page`, async ({ page }) => {
		await page.getByRole('navigation').getByRole('link', { name, exact: true }).click();
		await expect.poll(() => new URL(page.url()).pathname).toBe(path);
		const nav = page.getByRole('navigation');
		await expect(nav.getByRole('link', { name, exact: true })).toHaveAttribute(
			'aria-current',
			'page'
		);
		for (const other of labels) {
			if (other === name) continue;
			await expect(nav.getByRole('link', { name: other, exact: true })).not.toHaveAttribute(
				'aria-current'
			);
		}
	});
}
