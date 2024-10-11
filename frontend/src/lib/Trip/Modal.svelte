<script lang="ts">
	import { page } from '$app/stores';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import type { Route, Stop } from '$lib/static';
	import {
		trips as rt_trips,
		TripDirection,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';

	interface ModalProps {
		show_previous: boolean;
		trip: Trip<TrainTripData | BusTripData>;
	}

	const { trip, show_previous = $bindable() }: ModalProps = $props();

	// should this be in $derived?
	const route = $page.data.routes[trip.route_id];

	// if bus trip, add to monitored routes
	// $effect(() => {
	// 	if (route.route_type === 'bus') {
	// 		if (!monitored_routes.includes(route.id)) {
	// 			if (monitored_routes.length > 20) {
	// 				monitored_routes.shift();
	// 			}
	// 			monitored_routes.push(route.id);
	// 		}
	// 	}
	// });
</script>

<div class="flex gap-1 p-1">
	<!-- {#if large} -->

	<div class="font-medium text-lg">
		{trip.id}
	</div>

	<!-- <ModalList>
		{#each stop_times as st}
			<Button state={{ dialog_type: 'trip', data: st.trip }}>
				<div class="flex items-center"></div>
			</Button>
		{/each}
	</ModalList> -->
</div>
