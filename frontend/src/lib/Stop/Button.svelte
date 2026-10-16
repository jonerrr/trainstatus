<script lang="ts">
	import type { CatalogStop } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import { stop_arrivals } from '#lib/transit/arrivals.js';
	import { upcomingCalls } from '#lib/transit/decode.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { main_route_stops, trip_headsign } from '#lib/util.svelte.js';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		data: CatalogStop;
	}

	let { data: stop }: Props = $props();

	const source = $derived(stop.data.source);
	const catalog = $derived(transit.peekCatalog(source));
	const arrivals_resource = transit.stopArrivals(() => ({
		source,
		stop_id: stop.id,
		at: current_time.value
	}));
	const arrivals = $derived(arrivals_resource.current);
	const predictions = $derived.by(() => {
		const read = arrivals.value;
		const now = current_time.ms;
		return stop_arrivals(upcomingCalls(read?.calls ?? [], now), read?.routes ?? [], now);
	});
	const header_routes = $derived(
		main_route_stops(stop.routes, predictions.active_routes)
			.map((route) => catalog?.routes_by_id.get(route.route_id))
			.filter((route) => route !== undefined)
	);
	const directions = $derived(
		[1, 3].map((direction) => {
			const calls = predictions.arrivals.filter((call) => call.trip.direction === direction);
			const route_ids = [
				...new Set([
					...main_route_stops(stop.routes, new Set()).map((route) => route.route_id),
					...calls.map((call) => call.route_id)
				])
			];
			return {
				direction,
				routes: route_ids.map((route_id) => ({
					route_id,
					times: calls.filter((call) => call.route_id === route_id)
				}))
			};
		})
	);
</script>

{#snippet etas(times: import('#lib/transit/arrivals.js').StopArrival[], route_id: string)}
	{#if !arrivals.error && (arrivals.loading || arrivals.value?.pending.includes(route_id))}
		<span
			class="inline-block w-8 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
		<span
			class="inline-block w-10 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
	{:else}
		{#each times.slice(0, 2) as call (`${call.trip_id}:${call.sequence}`)}
			<span class="rounded-sm bg-neutral-800/70 px-1.5 py-0.5 text-sm font-medium"
				>{call.eta.toFixed(0)}m</span
			>
		{:else}
			<span class="text-neutral-400">—</span>
		{/each}
	{/if}
{/snippet}

{#if stop.data.source === 'mta_subway'}
	<div class="grid w-full grid-cols-1 gap-1">
		<div class="flex items-center gap-1">
			{#each header_routes as route (route.id)}
				<Icon height={24} width={24} link={false} {route} />
			{/each}

			<div class="my-auto text-left text-lg font-medium">
				{stop.name}
			</div>
		</div>
		<div class="grid grid-cols-2 gap-8">
			{#each directions as { direction, routes } (direction)}
				{const headsign = $derived(
					direction === 1 ? stop.data.north_headsign : stop.data.south_headsign
				)}
				<div class="mt-auto flex flex-col">
					<div class="table-cell max-w-[85%] text-left font-semibold">
						{headsign}
					</div>
					<div class="flex flex-col gap-1">
						{#each routes as { route_id, times } (route_id)}
							{const route = $derived(catalog?.routes_by_id.get(route_id))}
							<div class="flex items-center gap-1">
								{#if route}
									<Icon height={20} width={20} link={false} {route} />
								{/if}
								<div class="flex items-center gap-1">
									{@render etas(times, route_id)}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	</div>
{:else}
	<div class="flex flex-col text-white">
		<div class="flex items-center">
			{#if stop.data.source === 'mta_bus'}
				<div>
					<BusArrow direction={stop.data.direction} />
				</div>
			{/if}
			<div class="text-left text-lg font-medium">
				{stop.name}
			</div>
		</div>

		<div class="flex flex-col">
			{#each stop.routes as route_stop (route_stop.route_id)}
				{const route = $derived(catalog?.routes_by_id.get(route_stop.route_id))}
				{const calls = $derived(
					predictions.arrivals.filter((call) => call.route_id === route_stop.route_id)
				)}
				{const next = $derived(calls[0])}
				{const times = $derived(
					calls.filter((call) => call.trip.direction === next?.trip.direction)
				)}
				<div class="flex items-center gap-2 rounded-sm p-1 text-left text-wrap">
					{#if route}
						<Icon {route} link={false} width={32} height={32} />
					{/if}
					<div class="flex flex-col">
						<div class="min-h-6">
							{#if next}
								{trip_headsign(
									next.trip,
									route,
									arrivals.value?.routes.get(next.route_id)?.calls_by_trip.get(next.trip_id),
									catalog?.stops_by_id
								)}
							{:else}
								{('headsign' in route_stop.data && route_stop.data.headsign) || route?.long_name}
							{/if}
						</div>
						<div class="flex gap-2 pr-1">
							{@render etas(times, route_stop.route_id)}
						</div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div class="self-start text-neutral-300">
		#{stop.id}
	</div>
{/if}
