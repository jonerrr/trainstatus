import { expect, test } from './fixtures';

test('serves a CSP and renders representative routes without violations', async ({ page }) => {
	await page.addInitScript(() => {
		const storageKey = 'csp-violations';
		const violations = JSON.parse(sessionStorage.getItem(storageKey) ?? '[]') as string[];
		window.addEventListener('securitypolicyviolation', (event) => {
			violations.push(`${event.violatedDirective}: ${event.blockedURI}`);
			sessionStorage.setItem(storageKey, JSON.stringify(violations));
		});
	});

	for (const path of ['/', '/stops', '/charts', '/map', '/settings']) {
		const response = await page.goto(path);
		expect(response).not.toBeNull();
		expect(response?.headers()['content-security-policy']).toContain("default-src 'self'");
		await expect(page.getByRole('navigation')).toBeVisible();
	}

	expect(await page.evaluate(() => sessionStorage.getItem('csp-violations'))).toBeNull();
});
