<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';

	import { page } from '$app/state';

	import type { Stop } from '#lib/client/index.js';
	import { source_info } from '#lib/resources/index.svelte.js';
	import { awaitingRoute, awaitingRows } from '#lib/resources/pending.js';
	import { stop_time_context } from '#lib/resources/stop_times.svelte.js';
	import { trip_context } from '#lib/resources/trips.svelte.js';
	import Icon from '#lib/Route/Icon.svelte';
	import { get_stop_arrivals, type StopArrival } from '#lib/Stop/arrivals.js';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { main_route_stops, trip_headsign } from '#lib/util.svelte.js';

	const current_time = getCurrentTime();

	type StopTimesByRoute = SvelteMap<string, StopArrival[]>;

	interface Props {
		data: Stop;
		acquire_routes?: boolean;
	}

	let { data: stop, acquire_routes = true }: Props = $props();

	const routes = $derived(page.data.routes_by_id?.[stop.data.source] ?? {});

	const trips = $derived(trip_context.getSource(stop.data.source));
	const stop_times = $derived(stop_time_context.getSource(stop.data.source));

	$effect(() => {
		if (!acquire_routes || !source_info[stop.data.source]?.monitor_routes) return;
		return stop_times?.hold(stop.routes.map((route) => route.route_id));
	});

	const current_stop_times = $derived(stop_times?.current.by_stop_id.get(stop.id) ?? []);
	const { arrivals, active_routes } = $derived(
		get_stop_arrivals(current_stop_times, trips?.current, current_time.ms)
	);

	const is_loading = $derived(awaitingRows(stop_times, current_stop_times.length));

	// Express rows are added below only when an upcoming trip serves this direction.
	const main_rs = $derived(main_route_stops(stop.routes, new Set()));

	const stop_times_by_direction = $derived.by(() => {
		const stop_times_by_direction = new SvelteMap<number, StopTimesByRoute>();

		if (stop.data.source === 'mta_subway') {
			for (const direction of [1, 3]) {
				const route_map: StopTimesByRoute = new SvelteMap();
				for (const route of main_rs) {
					route_map.set(route.route_id, []);
				}
				stop_times_by_direction.set(direction, route_map);
			}
		}

		for (const st of arrivals) {
			const trip = st.trip;
			const route_id = trip.route_id;

			if (!stop_times_by_direction.has(trip.direction)) {
				stop_times_by_direction.set(trip.direction, new SvelteMap());
			}

			const target_map = stop_times_by_direction.get(trip.direction)!;
			if (!target_map.has(route_id)) {
				target_map.set(route_id, []);
			}

			target_map.get(route_id)!.push(st);
		}

		return stop_times_by_direction;
	});

	const default_stop_routes = $derived(
		main_route_stops(stop.routes, active_routes)
			.map((r) => routes[r.route_id])
			.filter((r) => r !== undefined)
	);

	// A bus stop sits on one side of the street, so it almost always serves a
	// single direction of a route (only ~63 of ~20k route/stop pairs see both).
	// Pick the direction of the next arrival so headsign and ETAs agree, rather
	// than mixing both directions into one row.
	function next_direction(route_id: string) {
		let best: { direction: number; times: StopArrival[] } | undefined;
		let best_eta = Infinity;

		for (const [direction, by_route] of stop_times_by_direction) {
			const times = by_route.get(route_id);
			if (!times?.length) continue;

			const soonest = Math.min(...times.map((st) => st.eta));
			if (soonest < best_eta) {
				best_eta = soonest;
				best = { direction, times };
			}
		}

		return best;
	}
</script>

{#snippet eta(n: number)}
	{const eta = $derived(parseInt(n.toFixed(0)))}
	{#key eta}
		<span class="rounded-sm bg-neutral-800/70 px-1.5 py-0.5 text-sm font-medium">
			{eta}m
		</span>
	{/key}
{/snippet}

{#snippet eta_or_loading(route_stop_times: StopArrival[], route_id: string)}
	{const loading = $derived(
		source_info[stop.data.source].monitor_routes ? awaitingRoute(stop_times, route_id) : is_loading
	)}
	{#if loading}
		<span
			class="inline-block w-8 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
		<span
			class="inline-block w-10 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
	{:else if stop_times?.error && !current_stop_times.length}
		<span class="text-xs text-amber-200">Unavailable</span>
	{:else if route_stop_times.length}
		{#each route_stop_times.slice(0, 2) as stop_time (stop_time.trip_id)}
			{@render eta(stop_time.eta)}
		{/each}
	{:else}
		<div class="text-neutral-400">No trips</div>
	{/if}
{/snippet}

{#if stop.data.source === 'mta_subway'}
	<div class="grid w-full grid-cols-1 gap-1">
		<div class="flex items-center gap-1">
			{#each default_stop_routes as route (route.id)}
				<Icon height={24} width={24} link={false} {route} />
			{/each}

			<div class="my-auto text-left text-lg font-medium">
				{stop.name}
			</div>
		</div>
		<div class="grid grid-cols-2 gap-8">
			{#each stop_times_by_direction as [direction, stop_times_by_route] (direction)}
				{const headsign = $derived(
					direction === 1 ? stop.data.north_headsign : stop.data.south_headsign
				)}
				<div class="mt-auto flex flex-col">
					<div class="table-cell max-w-[85%] text-left font-semibold">
						{headsign}
					</div>
					<div class="flex flex-col gap-1">
						{#each stop_times_by_route as [route_id, route_stop_times] (route_id)}
							{const route = $derived(routes[route_id])}
							<div class="flex items-center gap-1">
								<Icon height={20} width={20} link={false} {route} />
								<div class="flex items-center gap-1">
									{@render eta_or_loading(route_stop_times, route_id)}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	</div>
{:else if ['mta_bus', 'njt_bus'].includes(stop.data.source)}
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
				{const route = $derived(routes[route_stop.route_id])}
				{const next = $derived(next_direction(route_stop.route_id))}
				<!-- TODO: simplify this -->
				{const next_st = $derived(next?.times.reduce((a, b) => (a.eta <= b.eta ? a : b)))}
				{const next_trip = $derived(next_st?.trip)}
				<div class="flex items-center gap-2 rounded-sm p-1 text-left text-wrap">
					<Icon {route} link={false} width={32} height={32} />
					<div class="flex flex-col">
						<div>
							{#if next_trip}
								{trip_headsign(
									next_trip,
									route,
									stop_times?.current.by_trip_id.get(next_trip.id),
									page.data.stops_by_id[stop.data.source]
								)}
							{:else if 'headsign' in route_stop.data}
								{route_stop.data.headsign}
							{/if}
						</div>
						<div class="flex gap-2 pr-1">
							{@render eta_or_loading(next?.times ?? [], route_stop.route_id)}
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
