import { SvelteMap } from 'svelte/reactivity';

import type { Source } from '$lib/client';
import {
	LiveResource,
	type PositionResource,
	type PositionResources,
	type TypedVehiclePosition,
	createMultiSourceContext,
	source_info
} from '$lib/resources/index.svelte';
import { current_time } from '$lib/url_params.svelte';

export function index_positions<S extends Source>(
	data: TypedVehiclePosition<S>[]
): PositionResource<S> {
	return new SvelteMap(
		data.map((position) => [
			position.vehicle_id,
			{
				...position,
				updated_at: new Date(position.updated_at)
			}
		])
	);
}
export function createPositionResource<S extends Source>(source: S) {
	const resource = new LiveResource<PositionResource<S>>(
		async (signal) => {
			console.log(`updating ${source} positions`);

			const at = current_time.value;
			const query_params = at ? `?at=${at}` : '';
			const res = await fetch(`/api/v1/positions/${source}${query_params}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch vehicle positions');

			const data = (await res.json()) as TypedVehiclePosition<S>[];
			return index_positions<S>(data);
		},
		new SvelteMap(),
		{
			interval: source_info[source].refresh_interval.positions,
			debounce: 500
		}
	);

	let prev_time = current_time.value;
	$effect(() => {
		const val = current_time.value;
		if (val !== prev_time) {
			prev_time = val;
			resource.refresh();
		}
	});

	return resource;
}

export const position_context = createMultiSourceContext<PositionResources>();
// export const calculate_position_height = () => 80;
