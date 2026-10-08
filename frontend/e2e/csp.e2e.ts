import { expect, test } from './fixtures';

test('serves a CSP and renders representative routes without violations', async ({ page }) => {
	await page.addInitScript(() => {
		const storage_key = 'csp-violations';
		const violations = JSON.parse(sessionStorage.getItem(storage_key) ?? '[]') as string[];
		window.addEventListener('securitypolicyviolation', (event) => {
			violations.push(`${event.violatedDirective}: ${event.blockedURI}`);
			sessionStorage.setItem(storage_key, JSON.stringify(violations));
		});
	});

	for (const path of ['/', '/stops', '/charts', '/settings']) {
		const response = await page.goto(path);
		expect(response).not.toBeNull();
		expect(response?.headers()['content-security-policy']).toContain("default-src 'self'");
		await expect(page.getByRole('navigation')).toBeVisible();
	}

	expect(await page.evaluate(() => sessionStorage.getItem('csp-violations'))).toBeNull();
});
