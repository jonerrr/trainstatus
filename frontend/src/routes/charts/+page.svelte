<script lang="ts">
	import { LayerCake, Svg, Html, flatten } from 'layercake';
	import { scaleTime, scalePoint } from 'd3-scale';
	// import { timeParse, timeFormat } from 'd3-time-format';
	// import { line } from 'd3-shape';
	// import { extent } from 'd3-array';
	import { page } from '$app/state';
	import { TripDirection, trips } from '$lib/trips.svelte';
	import { monitored_bus_routes, stop_times } from '$lib/stop_times.svelte';
	import type { Route } from '$lib/static';
	import AxisX from './AxisX.svelte';
	import AxisY from './AxisY.svelte';
	import Lines from './Lines.svelte';
	import { Train } from 'lucide-svelte';

	let direction = $state<TripDirection>(TripDirection.North);
	let route = $state<Route>(page.data.routes['3']);

	$effect(() => {
		if (route.route_type === 'bus') {
			monitored_bus_routes.add(route.id);
		}
	});

	// TODO: filter for main stops or currently stopping there
	const route_stops = $derived.by(() => {
		if (!route) return;
		const route_stops = [];
		for (const stop of Object.values(page.data.stops)) {
			const route_at_stop = stop.routes.find((r) => r.id === route?.id);
			if (route_at_stop)
				route_stops.push({ id: stop.id, name: stop.name, sequence: route_at_stop.stop_sequence });
		}
		// sort by sequence
		route_stops.sort((a, b) => a.sequence - b.sequence);
		return route_stops;
	});
	// $inspect(route_stops);

	const formatted_data = $derived.by(() => {
		if (!trips.trips || !route) return;

		const route_trips = [];

		for (const trip of trips.trips.values()) {
			if (trip.route_id !== route.id || trip.direction !== direction) continue;
			const trip_st = stop_times.by_trip_id[trip.id];
			if (!trip_st) continue;
			const trip_points = trip_st.map((st) => {
				const stop = page.data.stops[st.stop_id];
				const stop_sequence = stop.routes.find((r) => r.id === trip.route_id)?.stop_sequence;
				return {
					trip_id: trip.id,
					route_id: trip.route_id,
					stop_id: st.stop_id,
					stop_name: stop.name,
					sequence: stop_sequence,
					time: st.arrival
				};
			});
			// .sort((a, b) => a.sequence - b.sequence);
			route_trips.push({ trip_id: trip.id, points: trip_points });
			// route_trips.push({ ...trip, stop_times: trip_st });
		}
		return route_trips;
	});

	// $inspect(formatted_data);
</script>

<svelte:head>
	<title>Charts | TrainStat.us</title>
</svelte:head>

<div class="flex flex-col gap-1 rounded">
	<div class="text-xl font-bold px-2">Charts</div>
	<div class="flex gap-4 bg-neutral-950 p-2 w-full items-start">
		<div class="grid grid-rows-3 gap-2">
			<div class="font-semibold">Direction</div>
			<div class="flex items-center">
				<input
					bind:group={direction}
					type="radio"
					id="northbound"
					name="direction"
					value={TripDirection.North}
					class="mr-2"
				/>
				<label for="northbound">Northbound</label>
			</div>
			<div class="flex items-center">
				<input
					bind:group={direction}
					type="radio"
					id="southbound"
					name="direction"
					value={TripDirection.South}
					class="mr-2"
				/>
				<label for="southbound">Southbound</label>
			</div>
		</div>
		<div class="flex flex-col gap-2">
			<div class="font-semibold">Route</div>
			<div class="relative w-64">
				<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
					<Train class="h-5 w-5 text-neutral-400" />
				</div>

				<select
					bind:value={route}
					class="block w-full rounded-md border border-neutral-700 bg-neutral-900 py-2 pl-10 pr-3 text-base focus:border-blue-500 focus:outline-none focus:ring-blue-500"
				>
					<option value="" disabled selected>Select a route</option>
					{#each Object.values(page.data.routes) as route}
						<option value={route}>
							{route.short_name}
						</option>
					{/each}
				</select>
			</div>
		</div>
	</div>
	<div class="w-[100dvw] h-[700px]">
		{#if formatted_data?.length && route_stops && route}
			<LayerCake
				debug
				padding={{ top: 20, right: 10, left: 160, bottom: 30 }}
				x="time"
				y="stop_name"
				yDomain={route_stops.map((d) => d.name)}
				yScale={scalePoint().padding(0.5)}
				xScale={scaleTime()}
				data={formatted_data}
				flatData={flatten(formatted_data, 'points')}
				custom={{
					stroke: '#' + route.color
				}}
			>
				<Svg>
					<AxisX />
					<AxisY />
					<Lines stroke="#{route.color}" />
				</Svg>
			</LayerCake>
		{/if}
	</div>
</div>

<style>
	/* Add any needed styles */
</style>
