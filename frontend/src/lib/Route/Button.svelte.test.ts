import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Button from './Button.svelte';

vi.mock('#lib/resources/alerts.svelte.js', () => ({
	alert_context: { get: () => ({}), getSource: () => undefined }
}));
test('route row content leaves activation to the enclosing list button', async () => {
	const view = await render(Button, {
		data: {
			id: '123',
			short_name: '123',
			long_name: 'Route 123',
			color: '#006699',
			text_color: '#ffffff',
			data: { source: 'njt_bus' }
		}
	});
	expect(view.container.querySelector('button,[role="button"]')).toBeNull();
	expect(view.container.textContent).toContain('123');
});
