<script lang="ts">
	import { page } from '$app/state';

	import type { Source, Stop, Transfer } from '#lib/client/index.js';
	import { COMPASS_DIRECTIONS } from '#lib/compassDirections.js';
	import { stop_time_context } from '#lib/resources/stop_times.svelte.js';
	import { trip_context } from '#lib/resources/trips.svelte.js';
	import Icon from '#lib/Route/Icon.svelte';
	import { get_stop_arrivals } from '#lib/Stop/arrivals.js';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';
	import { main_route_stops } from '#lib/util.svelte.js';

	const current_time = getCurrentTime();

	interface Props {
		stop_source: Source;
		transfers: Transfer[];
	}

	const { transfers, stop_source }: Props = $props();
	const all_trips = trip_context.get();
	const all_stop_times = stop_time_context.get();

	function transfer_routes(stop: Stop) {
		const source = stop.data.source;
		const { active_routes } = get_stop_arrivals(
			all_stop_times[source]?.current.by_stop_id.get(stop.id) ?? [],
			all_trips[source]?.current,
			current_time.ms
		);
		return main_route_stops(stop.routes, active_routes);
	}

	const sorted_transfers = $derived(
		transfers
			.map((t) => ({
				...t,
				stop: page.data.stops_by_id[t.to_stop_source]?.[t.to_stop_id]
			}))
			.filter((t): t is typeof t & { stop: Stop } => t.stop !== undefined)
			.sort((a, b) => {
				// Prioritize transfers from the same source as the current stop
				if (a.stop.data.source === stop_source && b.stop.data.source !== stop_source) return -1;
				if (b.stop.data.source === stop_source && a.stop.data.source !== stop_source) return 1;

				// Then sort by direction
				const dir_a =
					a.stop.data.source === 'mta_bus'
						? COMPASS_DIRECTIONS[a.stop.data.direction].transfer_order
						: COMPASS_DIRECTIONS.unknown.transfer_order;
				const dir_b =
					b.stop.data.source === 'mta_bus'
						? COMPASS_DIRECTIONS[b.stop.data.direction].transfer_order
						: COMPASS_DIRECTIONS.unknown.transfer_order;
				if (dir_a !== dir_b) return dir_a - dir_b;

				// Finally sort by route ID
				return a.stop.routes[0]?.route_id.localeCompare(b.stop.routes[0]?.route_id ?? '') ?? 0;
			})
	);
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center gap-2 overflow-x-auto">
		{#each sorted_transfers as { stop, to_stop_source } (stop.id)}
			<button
				class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
				onclick={() => open_modal({ type: 'stop', ...$state.snapshot(stop) })}
			>
				{#if stop.data.source === 'mta_bus'}
					<BusArrow direction={stop.data.direction} size="1rem" />
				{/if}
				{#each transfer_routes(stop) as route_stop (route_stop.route_id)}
					{const route = $derived(page.data.routes_by_id[to_stop_source]?.[route_stop.route_id])}
					{#if route}
						<Icon width={24} height={24} link={false} {route} />
					{/if}
				{/each}
			</button>
		{/each}
	</div>
</div>
