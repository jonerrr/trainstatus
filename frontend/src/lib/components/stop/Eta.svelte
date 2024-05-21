<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { Direction, type Route, type StopTime, type Trip } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import EtaTime from './EtaTime.svelte';

	dayjs.extend(relativeTime);

	export let stop_id: string;
	export let routes: Route[];
	export let trips: Trip[];
	interface RouteTrips {
		[key: string]: Trip[];
	}
	const route_trips: RouteTrips = {};
	routes.forEach((route) => {
		route_trips[route.id] = [];
	});
	trips.forEach((trip) => {
		route_trips[trip.route_id].push(trip);
	});
	// sort trips by route for easier access
	// let sorted_route_trips: RouteTrips = {};
	// Object.keys(route_trips).forEach((routeId) => {
	// 	sorted_route_trips[routeId] = route_trips[routeId].sort((a, b) => {
	// 		const aStopTime = a.stop_times.find((time) => time.stop_id === stop_id);
	// 		const bStopTime = b.stop_times.find((time) => time.stop_id === stop_id);
	// 		if (aStopTime && bStopTime) {
	// 			return new Date(aStopTime.arrival).getTime() - new Date(bStopTime.arrival).getTime();
	// 		}
	// 		return 0;
	// 	});
	// });

	// sort trips by route for easier access
	// let routes: Routes = {};

	// trips.forEach((trip) => {
	// 	if (!routes[trip.route_id]) {
	// 		routes[trip.route_id] = [];
	// 	}
	// 	routes[trip.route_id].push(trip);
	// });

	// console.log(routes);

	// each stop will need an array of this interface
	// interface Eta {
	// 	route_id: string;
	// 	direction: Direction;
	// 	eta: number;
	// }

	// function get_current_stop(times: StopTime[]) {
	// 	return times.find((time) => time.stop_id === stop_id);
	// }

	// const northbound = trips
	// 	.filter((trip) => trip.direction === Direction.North)
	// 	.slice(0, 3)
	// 	.map((trip) => {
	// 		return {
	// 			...trip,
	// 			eta: trip.stop_times.find((time) => time.stop_id === stop_id)
	// 		};
	// 	});
	console.log(route_trips);
</script>

<div class="flex flex-col gap-1">
	{#each routes as route}
		<div class="flex gap-2">
			<Icon name={route.id} />
			<div class="flex gap-1">
				{#each route_trips[route.id].slice(0, 2) as trip (trip.id)}
					<EtaTime stop_time={trip.stop_times.find((time) => time.stop_id === stop_id)} />
				{/each}
			</div>
		</div>
	{/each}
</div>
