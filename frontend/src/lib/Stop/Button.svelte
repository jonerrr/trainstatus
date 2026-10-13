<script lang="ts">
	import type { CatalogStop } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import { upcomingCalls } from '#lib/transit/decode.js';
	import { getTransitData, type StopArrivals } from '#lib/transit/store.svelte.js';
	import { stop_arrivals, type StopArrival } from '#lib/transit/view.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { main_route_stops, trip_headsign } from '#lib/util.svelte.js';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		data: CatalogStop;
	}

	let { data: stop }: Props = $props();

	const source = $derived(stop.data.source);
	const at = $derived(current_time.value ?? null);
	const catalog = $derived(transit.peekCatalog(source));
	// Rows render as arrivals load; a suspending read per row would batch them together.
	const arrivals_resource = transit.stopArrivals(() => ({ source, stop_id: stop.id, at }));
	const arrivals = $derived(arrivals_resource.current);

	interface View {
		by_direction: Map<number, Map<string, StopArrival[]>>;
		active_routes: ReadonlySet<string>;
		pending: ReadonlySet<string>;
		read: StopArrivals;
	}

	function view(read: StopArrivals): View {
		const now = current_time.ms;
		const upcoming = upcomingCalls(read.calls, now);
		const { arrivals, active_routes } = stop_arrivals(upcoming, read.routes, now);

		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		const by_direction = new Map<number, Map<string, StopArrival[]>>();
		if (source === 'mta_subway') {
			// Express rows appear only when an upcoming trip serves this direction.
			const main = main_route_stops(stop.routes, new Set());
			for (const direction of [1, 3]) {
				by_direction.set(direction, new Map(main.map((route) => [route.route_id, []])));
			}
		}
		for (const st of arrivals) {
			let routes = by_direction.get(st.trip.direction);
			// eslint-disable-next-line svelte/prefer-svelte-reactivity
			if (!routes) by_direction.set(st.trip.direction, (routes = new Map()));
			let times = routes.get(st.route_id);
			if (!times) routes.set(st.route_id, (times = []));
			times.push(st);
		}
		return { by_direction, active_routes, pending: new Set(read.pending), read };
	}

	// A bus stop sits on one side of the street, so it almost always serves a
	// single direction of a route (only ~63 of ~20k route/stop pairs see both).
	// Pick the direction of the next arrival so headsign and ETAs agree.
	function next_arrival(v: View, route_id: string): StopArrival | undefined {
		let best: StopArrival | undefined;
		for (const routes of v.by_direction.values()) {
			const first = routes.get(route_id)?.[0];
			if (first && (!best || first.eta < best.eta)) best = first;
		}
		return best;
	}

	function next_times(v: View, route_id: string): StopArrival[] {
		const next = next_arrival(v, route_id);
		return next ? (v.by_direction.get(next.trip.direction)?.get(route_id) ?? []) : [];
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

{#snippet etas(v: View | undefined, times: StopArrival[], route_id: string, failed = false)}
	{#if !failed && (!v || v.pending.has(route_id))}
		<span
			class="inline-block w-8 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
		<span
			class="inline-block w-10 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
	{:else if times.length}
		{#each times.slice(0, 2) as st (st.trip_id)}
			{@render eta(st.eta)}
		{/each}
	{:else}
		<span class="text-neutral-400">—</span>
	{/if}
{/snippet}

{#snippet body(v: View | undefined, failed = false)}
	{#if stop.data.source === 'mta_subway'}
		{const header_routes = $derived(
			main_route_stops(stop.routes, v?.active_routes ?? new Set())
				.map((r) => catalog?.routes_by_id.get(r.route_id))
				.filter((r) => r !== undefined)
		)}
		{const directions = $derived(
			v?.by_direction ??
				new Map(
					[1, 3].map((direction) => [
						direction,
						new Map(
							main_route_stops(stop.routes, new Set()).map((r) => [r.route_id, [] as StopArrival[]])
						)
					])
				)
		)}
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
				{#each directions as [direction, by_route] (direction)}
					{const headsign = $derived(
						direction === 1 ? stop.data.north_headsign : stop.data.south_headsign
					)}
					<div class="mt-auto flex flex-col">
						<div class="table-cell max-w-[85%] text-left font-semibold">
							{headsign}
						</div>
						<div class="flex flex-col gap-1">
							{#each by_route as [route_id, times] (route_id)}
								{const route = $derived(catalog?.routes_by_id.get(route_id))}
								<div class="flex items-center gap-1">
									{#if route}
										<Icon height={20} width={20} link={false} {route} />
									{/if}
									<div class="flex items-center gap-1">
										{@render etas(v, times, route_id, failed)}
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
					{const next = $derived(v && next_arrival(v, route_stop.route_id))}
					<div class="flex items-center gap-2 rounded-sm p-1 text-left text-wrap">
						{#if route}
							<Icon {route} link={false} width={32} height={32} />
						{/if}
						<div class="flex flex-col">
							<div>
								{#if next && v}
									{trip_headsign(
										next.trip,
										route,
										v.read.routes.get(next.route_id)?.calls_by_trip.get(next.trip_id),
										catalog?.stops_by_id
									)}
								{:else if 'headsign' in route_stop.data}
									{route_stop.data.headsign}
								{/if}
							</div>
							<div class="flex gap-2 pr-1">
								{@render etas(
									v,
									v ? next_times(v, route_stop.route_id) : [],
									route_stop.route_id,
									failed
								)}
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
{/snippet}

{@render body(arrivals.value && view(arrivals.value), !!arrivals.error)}
