import type { ApiAlert } from '$lib/client';

import { describe, expect, it } from 'vitest';

import { index_alerts } from './alerts.svelte';

function alert(id: string, entities: ApiAlert['entities']): ApiAlert {
	return {
		id,
		original_id: id,
		data: { source: 'mta_subway', alert_type: 'Planned Work', display_before_active: 0 },
		entities,
		translations: [],
		created_at: new Date('2026-09-07T00:00:00Z'),
		updated_at: new Date('2026-09-07T00:00:00Z'),
		start_time: new Date('2026-09-07T00:00:00Z')
	};
}

describe('index_alerts', () => {
	it('lists an alert once per route even when it affects multiple stops', () => {
		const input = alert('planned-work', [
			{ route_id: '4', stop_id: '415', sort_order: 20 },
			{ route_id: '4', stop_id: '416', sort_order: 20 },
			{ route_id: '5', stop_id: '415', sort_order: 20 }
		]);
		const result = index_alerts([input]);

		expect(result.alerts_by_route.get('4')).toEqual([result.alerts[0]]);
		expect(result.alerts_by_route.get('5')).toEqual([result.alerts[0]]);
		expect(result.alerts[0].entities).toEqual(input.entities);
	});

	it('keeps distinct alerts on the same route and rebuilds the index on refresh', () => {
		const input = [
			alert('first', [{ route_id: '4', sort_order: 20 }]),
			alert('second', [{ route_id: '4', sort_order: 16 }])
		];
		const result = index_alerts(input);
		const refreshed = index_alerts(input);

		expect(result.alerts_by_route.get('4')?.map((item) => item.id)).toEqual(['first', 'second']);
		expect(refreshed.alerts_by_route.get('4')).toHaveLength(2);
		expect(index_alerts([]).alerts_by_route.size).toBe(0);
	});
});
