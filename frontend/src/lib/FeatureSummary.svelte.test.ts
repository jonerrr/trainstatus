import '../app.css';

import type { Route } from '#lib/client/index.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import FeatureSummary from './FeatureSummary.svelte';

vi.mock('#lib/resources/alerts.svelte.js', () => ({
	alert_context: { get: () => ({}) }
}));

for (const label of ['Q114-LTD', 'SIM34', '123X']) {
	test(`route badge ${label} fits its text in a narrow feature card`, async () => {
		const route: Route = {
			id: label,
			short_name: label,
			long_name: 'A long bus route destination',
			color: '#006699',
			text_color: '#ffffff',
			data: { source: 'njt_bus' }
		};
		const view = await render(FeatureSummary, {
			feature: {
				route,
				title: 'A long destination with multiple words and a transfer station',
				subtitle: 'NJ Transit Bus'
			}
		});
		view.container.style.width = '240px';
		const badge = view.container.querySelector<HTMLElement>('.route-badge')!;
		const range = document.createRange();
		range.selectNodeContents(badge);
		const text = range.getBoundingClientRect(),
			box = badge.getBoundingClientRect();
		expect(text.left).toBeGreaterThanOrEqual(box.left);
		expect(text.right).toBeLessThanOrEqual(box.right);
		expect(view.container.scrollWidth).toBeLessThanOrEqual(240);
		expect(badge.textContent).toContain(label);
	});
}
