import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Button from './Button.svelte';

test('route row content leaves activation to the enclosing list button', async () => {
	const view = await render(TransitProvider, {
		props: {
			data: fixtureTransitData({ catalogs: {}, services: {} }),
			component: Button as never,
			child_props: {
				data: {
					id: '123',
					short_name: '123',
					long_name: 'Route 123',
					color: '#006699',
					text_color: '#ffffff',
					data: { source: 'njt_bus' }
				}
			}
		}
	});
	expect(view.container.querySelector('button,[role="button"]')).toBeNull();
	expect(view.container.textContent).toContain('123');
});
