<script lang="ts">
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import BusArrow from '$lib/Stop/BusArrow.svelte';
	import type { Stop, StopTime } from '$lib/client';
	import { source_info } from '$lib/resources/index.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { current_time } from '$lib/url_params.svelte';
	import { main_route_stops } from '$lib/util.svelte';

	type StopTimeWithETA = StopTime & { eta: number };
	type StopTimesByRoute = Map<string, StopTimeWithETA[]>;

	interface Props {
		data: Stop;
	}

	let { data: stop }: Props = $props();

	const routes = $derived(page.data.routes_by_id?.[stop.data.source] ?? {});

	const trips = $derived(trip_context.getSource(stop.data.source));
	const stop_times = $derived(stop_time_context.getSource(stop.data.source));

	$effect(() => {
		if (stop_times && source_info[stop.data.source]?.monitor_routes) {
			for (const r of stop.routes) {
				stop_times.add_route(r.route_id);
			}
		}
	});

	const current_stop_times = $derived(stop_times?.current.by_stop_id.get(stop.id) ?? []);

	const is_loading = $derived(
		!stop_times || (stop_times.status !== 'ready' && current_stop_times.length === 0)
	);

	const main_rs = $derived(main_route_stops(stop.routes));

	const stop_times_by_direction = $derived.by(() => {
		const stop_times_by_direction = new Map<number, StopTimesByRoute>();

		if (stop.data.source === 'mta_subway') {
			for (const direction of [1, 3]) {
				const route_map: StopTimesByRoute = new Map();
				for (const route of main_rs) {
					route_map.set(route.route_id, []);
				}
				stop_times_by_direction.set(direction, route_map);
			}
		}

		for (const st of current_stop_times) {
			if (st.arrival.getTime() < current_time.ms) continue;
			const trip = trips?.current?.get(st.trip_id);
			if (!trip) continue;

			const route_id = trip.route_id;

			if (!stop_times_by_direction.has(trip.direction)) {
				stop_times_by_direction.set(trip.direction, new Map());
			}

			const target_map = stop_times_by_direction.get(trip.direction)!;
			if (!target_map.has(route_id)) {
				target_map.set(route_id, []);
			}

			const eta = (st.arrival.getTime() - current_time.ms) / 1000 / 60;

			target_map.get(route_id)!.push({ ...st, eta });
		}

		return stop_times_by_direction;
	});

	const default_stop_routes = $derived(
		main_rs.map((r) => routes[r.route_id]).filter((r) => r !== undefined)
	);
</script>

{#snippet eta(n: number)}
	{@const eta = parseInt(n.toFixed(0))}
	{#key eta}
		<span class="rounded-sm bg-neutral-800/70 px-1.5 py-0.5 text-sm font-medium">
			{eta}m
		</span>
	{/key}
{/snippet}

{#snippet eta_or_loading(route_stop_times: StopTimeWithETA[])}
	{#if is_loading}
		<span
			class="inline-block w-8 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
		<span
			class="inline-block w-10 animate-pulse rounded-sm bg-neutral-800 px-1.5 py-0.5 text-sm leading-5"
			>&nbsp;</span
		>
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
				{@const headsign = direction === 1 ? stop.data.north_headsign : stop.data.south_headsign}
				<div class="mt-auto flex flex-col">
					<div class="table-cell max-w-[85%] text-left font-semibold">
						{headsign}
					</div>
					<div class="flex flex-col gap-1">
						{#each stop_times_by_route as [route_id, route_stop_times] (route_id)}
							{@const route = routes[route_id]}
							<div class="flex items-center gap-1">
								<Icon height={20} width={20} link={false} {route} />
								<div class="flex items-center gap-1">
									{@render eta_or_loading(route_stop_times)}
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
				{@const route = routes[route_stop.route_id]}
				{@const route_stop_times = [...stop_times_by_direction.values()].flatMap(
					(m) => m.get(route_stop.route_id) ?? []
				)}
				{#if !route}
					{@debug stop}
				{/if}
				<div class="flex items-center gap-2 rounded-sm p-1 text-left text-wrap">
					<Icon {route} link={false} />
					<div class="flex flex-col">
						<div>
							{#if 'headsign' in route_stop.data}
								{route_stop.data.headsign}
							{/if}
						</div>
						<div class="flex gap-2 pr-1">
							{@render eta_or_loading(route_stop_times)}
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
