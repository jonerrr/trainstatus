import type { ApiAlert } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import { decodeAlerts } from './alerts.js';

test('alerts retain Unix seconds and appear once per route after icon rendering', () => {
	const alert: ApiAlert = {
		id: 'alert-1',
		original_id: 'provider-1',
		data: { source: 'njt_bus' },
		created_at: 0,
		updated_at: 100,
		start_time: 50,
		end_time: 0,
		translations: [{ section: 'header', language: 'en', format: 'html', text: '[A] delay' }],
		entities: [
			{ route_id: '87', sort_order: 1 },
			{ route_id: '87', stop_id: '100', sort_order: 2 }
		]
	};
	const decoded = decodeAlerts(JSON.parse(JSON.stringify([alert])));
	expect(decoded.alerts[0]).toMatchObject({
		created_at: 0,
		updated_at: 100,
		start_time: 50,
		end_time: 0
	});
	expect(decoded.by_route.get('87')).toHaveLength(1);
	expect(decoded.alerts[0].translations[0].text).toContain('<svg');
	expect(alert.translations[0].text).toBe('[A] delay');
	expect(decodeAlerts([{ ...alert, end_time: undefined }]).alerts[0].end_time).toBeUndefined();
});
