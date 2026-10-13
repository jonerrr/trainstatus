import { expect, test } from './fixtures';

test('source preferences persist after a reload and keep at least one source enabled', async ({
	page
}) => {
	await page.goto('/settings');
	const subway = page.getByRole('checkbox', { name: 'MTA Subway' });
	const bus = page.getByRole('checkbox', { name: 'MTA Bus', exact: true });
	const njt = page.getByRole('checkbox', { name: 'NJT Bus' });
	await expect(subway).toBeChecked();
	await expect(bus).toBeChecked();
	await expect(njt).not.toBeChecked();
	await bus.uncheck();
	await expect(subway).toBeDisabled();
	await page.reload();
	await expect(bus).not.toBeChecked();
	await expect(subway).toBeChecked();
	await expect(subway).toBeDisabled();
});

test('all agency icons load at their displayed size', async ({ page }) => {
	await page.goto('/settings');
	const images = page.locator('main label img');
	await expect(images).toHaveCount(3);
	for (const image of await images.all()) {
		await expect(image).toBeVisible();
		await expect
			.poll(() =>
				image.evaluate((element: HTMLImageElement) => element.complete && element.naturalWidth > 0)
			)
			.toBe(true);
		// Catch accidentally shipping the original full-size agency artwork.
		const width = await image.evaluate((element: HTMLImageElement) => element.naturalWidth);
		expect(width).toBeLessThanOrEqual(72);
	}
});

test('applying the time sets at and a reload keeps it', async ({ page }) => {
	await page.goto('/settings');
	await page.locator('input[type="datetime-local"]').fill('2026-10-01T12:00');
	await expect(page).toHaveURL(/\/settings$/);
	await page.getByRole('button', { name: 'Apply', exact: true }).click();
	await expect(page).toHaveURL(/[?&]at=\d+/);
	const at = new URL(page.url()).searchParams.get('at');
	expect(at).toBeTruthy();
	await expect(page.getByRole('link', { name: 'Home', exact: true })).toHaveAttribute(
		'href',
		`/?at=${at}`
	);
	await page.getByRole('link', { name: 'Home', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`\\/\\?at=${at}$`));
	await page.getByRole('link', { name: 'Settings', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`\\/settings\\?at=${at}$`));
	await page.reload();
	await expect(page).toHaveURL(new RegExp(`[?&]at=${at}(?:&|$)`));
	await page.getByRole('button', { name: 'Return to live data' }).click();
	await expect(page).toHaveURL(/\/settings$/);
});
