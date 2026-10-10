import { SvelteDate, SvelteMap } from 'svelte/reactivity';

import type { Source } from '#lib/client/index.js';
import {
	createEntityResource,
	createMultiSourceContext,
	type PositionResource,
	type PositionResources,
	type TypedVehiclePosition
} from '#lib/resources/index.svelte.js';

export function index_positions<S extends Source>(
	data: TypedVehiclePosition<S>[]
): PositionResource<S> {
	return new SvelteMap(
		data.map((position) => [
			position.vehicle_id,
			{
				...position,
				updated_at: new SvelteDate(position.updated_at)
			}
		])
	);
}
export function createPositionResource<S extends Source>(source: S) {
	return createEntityResource<TypedVehiclePosition<S>[], PositionResource<S>>(
		source,
		'positions',
		index_positions<S>,
		new SvelteMap()
	);
}

export const position_context = createMultiSourceContext<PositionResources>();
// export const calculate_position_height = () => 80;
