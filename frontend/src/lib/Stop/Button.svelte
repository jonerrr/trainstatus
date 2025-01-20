<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { page } from '$app/state';
	import { is_bus, is_train, main_stop_routes, type Route, type Stop } from '$lib/static';
	import { stop_times as rt_stop_times, type StopTime } from '$lib/stop_times.svelte';
	import { trips as rt_trips, TripDirection } from '$lib/trips.svelte';
	import { debounce, current_time } from '$lib/util.svelte';
	import BusArrow from './BusArrow.svelte';
	import Icon from '$lib/Icon.svelte';

	interface Props {
		data: Stop<'train' | 'bus'>;
	}
	let { data }: Props = $props();

	interface StopTimeData extends StopTime {
		eta: number;
		direction: TripDirection;
		route_id: string;
	}

	type StopTimeByRoute = Map<string, StopTimeData[]>;

	const nb_st_by_route = $state<StopTimeByRoute>(new SvelteMap());
	const sb_st_by_route = $state<StopTimeByRoute>(new SvelteMap());
	const active_routes = $state(new SvelteSet<Route>());

	$effect(() => {
		rt_stop_times?.by_stop_id;
		rt_trips.trips;
		data;

		// console.log('Triggering update for', data.name);
		untrack(() => updateRouteMaps());
	});

	// Debounced update function
	const updateRouteMaps = debounce(() => {
		// Clear existing maps
		nb_st_by_route.clear();
		sb_st_by_route.clear();

		// Early exit if no data
		// if (!rt_stop_times?.stop_times?.length) {
		// 	return;
		// }
		const stop_times = rt_stop_times.by_stop_id[data.id] ?? [];

		for (const st of stop_times) {
			if (st.arrival.getTime() < current_time.ms) continue;
			const trip = rt_trips.trips.get(st.trip_id);
			if (!trip) continue;

			const route_id = trip.route_id;
			active_routes.add(page.data.routes[route_id]);

			const eta = (st.arrival.getTime() - current_time.ms) / 1000 / 60;

			const stopTimeData = {
				...st,
				eta,
				direction: trip.direction,
				route_id
			};

			if (is_train(data)) {
				const target_map = trip.direction === TripDirection.North ? nb_st_by_route : sb_st_by_route;

				if (!target_map.has(route_id)) {
					target_map.set(route_id, []);
				}

				const times = target_map.get(route_id)!;
				if (times.length < 2) {
					times.push(stopTimeData);
				}
			} else {
				if (!nb_st_by_route.has(route_id)) {
					nb_st_by_route.set(route_id, []);
				}

				const times = nb_st_by_route.get(route_id)!;
				if (times.length < 2) {
					times.push(stopTimeData);
				}
			}
		}
	}, 35);

	// TODO: fix 1 train showing up at dekalb ave for some reason
	const current_stop_routes = $derived(main_stop_routes(data).map((r) => page.data.routes[r.id]));
	// combine current stop routes with other active routes at stop
	// TODO: use nb_st_by_route and sb_st_by_route to get active routes instead of creating new set
	const all_stop_routes = $derived([...new Set(current_stop_routes.concat(...active_routes))]);
	// const all_stop_routes = $derived(current_stop_routes);
</script>

{#snippet eta(n: number)}
	{@const eta = parseInt(n.toFixed(0))}
	{#key eta}
		<!-- in:fade={{ duration: 300 }} -->
		<span>
			{eta}m
		</span>
	{/key}
{/snippet}

{#if is_train(data)}
	<div class="grid gap-1 w-full grid-cols-1">
		<div class="flex gap-1 items-center">
			{#each current_stop_routes as route (route.id)}
				<Icon height={24} width={24} express={false} link={false} {route} />
			{/each}

			<div class="font-medium my-auto text-left text-lg">
				{data.name}
			</div>
		</div>
		<div class="grid grid-cols-2 gap-8">
			{@render arrivals(data.data.north_headsign, all_stop_routes, nb_st_by_route)}
			{@render arrivals(data.data.south_headsign, all_stop_routes, sb_st_by_route)}
		</div>
	</div>

	{#snippet arrivals(headsign: string, routes: Route[], stop_times: StopTimeByRoute)}
		<div class="flex flex-col mt-auto">
			<div class="font-semibold table-cell text-left max-w-[85%]">
				{headsign}
			</div>
			<div class="flex flex-col gap-1">
				{#each routes as route (route.id)}
					{@const route_stop_times = stop_times.get(route.id) ?? []}
					<!-- {@const route_stop_times = stop_times.filter((st) => st.route_id === route.id)} -->
					<div class="flex gap-1 items-center">
						<Icon height={20} width={20} express={false} link={false} {route} />
						<div class="flex gap-1 items-center">
							{#if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time (stop_time.trip_id)}
									{@render eta(stop_time.eta)}
								{/each}
							{:else}
								<div class="text-neutral-400">No trips</div>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/snippet}
{:else if is_bus(data)}
	<!-- TODO: make spacing consistent (use grid maybe idk) -->
	<div class="flex flex-col text-white">
		<div class="flex items-center">
			<div>
				<BusArrow direction={data.data.direction} />
			</div>
			<div class="font-medium text-lg text-left">
				{data.name}
			</div>
		</div>

		<div class="flex flex-col">
			{#each data.routes as stop_route (stop_route.id)}
				{@const route = page.data.routes[stop_route.id] as Route}
				<!-- {@const route_stop_times = stop_times.filter((st) => st.route_id === stop_route.id)} -->
				{@const route_stop_times = nb_st_by_route.get(stop_route.id) ?? []}

				<div class="flex gap-2 items-center text-wrap text-left rounded p-1">
					<Icon {route} link={false} express={false} />
					<div class="flex flex-col">
						<div class="">
							{stop_route.headsign}
						</div>
						<div class="flex gap-2 pr-1">
							{#if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time (stop_time.trip_id)}
									{@render eta(stop_time.eta)}
								{/each}
								<!-- check if trips contains trip with route_id and that there are no stop times -->
								<!-- {:else if stop_times.length && !route_stop_times.length}
									<div class="text-neutral-400">Loading...</div> -->
							{:else}
								<div class="text-neutral-400">No trips</div>
							{/if}
						</div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div class="text-neutral-300 self-start">
		#{data.id}
	</div>
{/if}
