<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop, Source, Transfer } from '#lib/client/index.js';
	import { COMPASS_DIRECTIONS } from '#lib/compassDirections.js';
	import Icon from '#lib/Route/Icon.svelte';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import { getTransitData, loaded } from '#lib/transit/store.svelte.js';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';
	import { main_route_stops } from '#lib/util.svelte.js';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		stop_source: Source;
		transfers: Transfer[];
	}

	const { transfers, stop_source }: Props = $props();

	// Badges use only routes already loaded; transfers never request service.
	function transfer_routes(stop: CatalogStop) {
		const now = current_time.ms;
		const active_routes = new Set(
			transit
				.loadedStopArrivals(stop.data.source, stop.id, current_time.value ?? null)
				.calls.filter((call) => call.arrival >= now)
				.map((call) => call.route_id)
		);
		return main_route_stops(stop.routes, active_routes);
	}

	const direction_order = (stop: CatalogStop) =>
		stop.data.source === 'mta_bus'
			? COMPASS_DIRECTIONS[stop.data.direction].transfer_order
			: COMPASS_DIRECTIONS.unknown.transfer_order;

	const sorted_transfers = $derived(
		transfers
			.filter((t) => page.data.selected_sources.includes(t.to_stop_source))
			.map((t) => ({
				...t,
				stop: loaded(transit.catalog(t.to_stop_source))?.stops_by_id.get(t.to_stop_id)
			}))
			.filter((t): t is typeof t & { stop: CatalogStop } => t.stop !== undefined)
			.sort((a, b) => {
				// Prioritize transfers from the same source as the current stop
				if (a.stop.data.source === stop_source && b.stop.data.source !== stop_source) return -1;
				if (b.stop.data.source === stop_source && a.stop.data.source !== stop_source) return 1;

				const dir_a = direction_order(a.stop);
				const dir_b = direction_order(b.stop);
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
				onclick={() => open_modal({ type: 'stop', source: to_stop_source, id: stop.id })}
			>
				{#if stop.data.source === 'mta_bus'}
					<BusArrow direction={stop.data.direction} size="1rem" />
				{/if}
				{#each transfer_routes(stop) as route_stop (route_stop.route_id)}
					{const route = $derived(
						loaded(transit.catalog(to_stop_source))?.routes_by_id.get(route_stop.route_id)
					)}
					{#if route}
						<Icon width={24} height={24} link={false} {route} />
					{/if}
				{/each}
			</button>
		{/each}
	</div>
</div>
