import { SvelteMap } from 'svelte/reactivity';

import { LiveResource, createMultiSourceContext, source_info } from '$lib/sources/index.svelte';

import type { Source, VehiclePosition } from '@trainstatus/client';

export type PositionResource = SvelteMap<string, VehiclePosition>;

//TODO: compare map and for loop performance
export function index_positions(data: VehiclePosition[]): PositionResource {
	// TODO: maybe index by trip_id instead, since vehicles can be associated with multiple trips
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

export function createPositionResource(
	source: Source,
	params: { at?: number },
	initial_value: PositionResource
) {
	const resource = new LiveResource<PositionResource>(
		async (signal) => {
			console.log('updating vehicle positions');
			const query = new URLSearchParams();
			if (params.at) query.set('at', params.at.toString());

			const res = await fetch(`/api/v1/positions/${source}?${query}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch vehicle positions');

			const data: VehiclePosition[] = await res.json();
			return index_positions(data);
		},
		{
			initial_value,
			interval: source_info[source].refresh_interval.positions,
			debounce: 500 // TODO: increase time
		}
	);

	$effect(() => {
		if (params.at !== undefined) {
			resource.refresh();
		}
	});

	return resource;
}

export const positions_context =
	createMultiSourceContext<ReturnType<typeof createPositionResource>>('positions');

// export const calculate_position_height = () => 80;
