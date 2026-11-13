<script lang="ts">
	import type { CatalogStop } from '#lib/client/index.js';
	import { stopArrivals, type StopArrival } from '#lib/data/arrivals.js';
	import { upcomingCalls } from '#lib/data/decode.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import Icon from '#lib/route/Icon.svelte';
	import BusArrow from '#lib/stop/BusArrow.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';
	import { busHeadsign, mainRouteStops, tripHeadsign } from '#lib/util.svelte.js';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		data: CatalogStop;
	}

	let { data: stop }: Props = $props();

	const source = $derived(stop.data.source);
	const catalog = $derived(transit.peekCatalog(source));
	const arrivalsResource = transit.stopArrivals(() => ({
		source,
		stopId: stop.id,
		at: currentTime.value
	}));
	const arrivals = $derived(arrivalsResource.current);
	const predictions = $derived.by(() => {
		const read = arrivals.state === 'ready' ? arrivals.value : undefined;
		const now = currentTime.ms;
		return stopArrivals(upcomingCalls(read?.calls ?? [], now), read?.routes ?? [], now);
	});
	const headerRoutes = $derived(
		mainRouteStops(stop.routes, predictions.activeRoutes)
			.map((route) => catalog?.routesById.get(route.route_id))
			.filter((route) => route !== undefined)
	);
	const directions = $derived(
		[1, 3].map((direction) => {
			const calls = predictions.arrivals.filter((call) => call.trip.direction === direction);
			const routeIds = [
				...new Set([
					...mainRouteStops(stop.routes, new Set()).map((route) => route.route_id),
					...calls.map((call) => call.routeId)
				])
			];
			return {
				direction,
				routes: routeIds.map((routeId) => ({
					routeId,
					times: calls.filter((call) => call.routeId === routeId)
				}))
			};
		})
	);
</script>

{#snippet etas(times: StopArrival[], routeId: string)}
	{#if arrivals.state === 'loading' || (arrivals.state === 'ready' && arrivals.value.pending.includes(routeId))}
		<span
			class="inline-block w-8 animate-pulse rounded-none bg-surface-2 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
		<span
			class="inline-block w-10 animate-pulse rounded-none bg-surface-2 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
	{:else}
		{#each times.slice(0, 2) as call (`${call.tripId}:${call.sequence}`)}
			<span class="rounded-none bg-surface-2/70 px-1.5 py-0.5 text-sm font-medium"
				>{call.eta.toFixed(0)}m</span
			>
		{:else}
			<span class="text-muted">—</span>
		{/each}
	{/if}
{/snippet}

{#if stop.data.source === 'mta_subway'}
	<div class="grid w-full grid-cols-1 gap-1">
		<div class="flex items-center gap-1">
			{#each headerRoutes as route (route.id)}
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
						{#each routes as { routeId, times } (routeId)}
							{const route = $derived(catalog?.routesById.get(routeId))}
							<div class="flex items-center gap-1">
								{#if route}
									<Icon height={20} width={20} link={false} {route} />
								{/if}
								<div class="flex items-center gap-1">
									{@render etas(times, routeId)}
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
			{#each stop.routes as routeStop (routeStop.route_id)}
				{const route = $derived(catalog?.routesById.get(routeStop.route_id))}
				{const calls = $derived(
					predictions.arrivals.filter((call) => call.routeId === routeStop.route_id)
				)}
				{const next = $derived(calls[0])}
				{const labelCount =
					routeStop.data.source === 'mta_bus'
						? Math.max(1, routeStop.data.direction_ids.length)
						: routeStop.data.source === 'njt_bus'
							? Math.max(1, routeStop.data.directions.length)
							: 1}
				{const times = $derived(
					calls.filter((call) => call.trip.direction === next?.trip.direction)
				)}
				<div class="flex items-center gap-2 rounded-none p-1 text-left text-wrap">
					{#if route}
						<Icon {route} link={false} width={32} height={32} />
					{/if}
					<div class="flex flex-col">
						<div style:min-height={`${labelCount * 24}px`}>
							{#if next}
								{tripHeadsign(
									next.trip,
									route,
									arrivals.state === 'ready'
										? arrivals.value.routes.get(next.routeId)?.callsByTrip.get(next.tripId)
										: undefined,
									catalog?.stopsById
								)}
							{:else if routeStop.data.source === 'mta_bus'}
								{#each routeStop.data.direction_ids as direction (direction)}
									<div>{busHeadsign(route, direction)}</div>
								{/each}
							{:else if routeStop.data.source === 'njt_bus'}
								{#each routeStop.data.directions as direction (direction.direction_id)}<div>
										{direction.headsign || route?.long_name}
									</div>{/each}
							{/if}
						</div>
						<div class="flex gap-2 pr-1">
							{@render etas(times, routeStop.route_id)}
						</div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div class="self-start text-secondary">
		#{stop.id}
	</div>
{/if}
