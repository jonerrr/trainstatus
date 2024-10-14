<script lang="ts">
	// import { slide } from 'svelte/transition';
	import { page } from '$app/stores';
	import { type StopTime, stop_times } from '$lib/stop_times.svelte';
	import {
		is_train_route,
		trips,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';

	const { stop_time, trip }: { stop_time: StopTime; trip: Trip<BusTripData | TrainTripData> } =
		$props();

	// get all trips that stop at this stop after this stop time
	const transfers = $derived.by(() => {
		const transfers = [];

		for (const st of stop_times.stop_times) {
			if (
				st.trip_id === stop_time.trip_id ||
				st.stop_id !== stop_time.stop_id ||
				st.arrival < stop_time.arrival
			) {
				continue;
			}
			if (transfers.length >= 3) {
				break;
			}

			const transfer_trip = trips.trips.get(st.trip_id)!;
			// nobody wants to transfer to the same route
			if (transfer_trip.route_id === trip.route_id || transfer_trip.direction !== trip.direction) {
				continue;
			}

			transfers.push({ ...st, trip: transfer_trip });
		}

		return transfers;
	});
</script>

<div class="flex flex-col px-1 bg-neutral-900">
	<div class="font-medium">Transfers:</div>
	<div class="flex gap-2 items-center justify-evenly">
		{#if transfers.length}
			{#each transfers as st}
				{@const route = $page.data.routes[st.trip.route_id]}
				<div class="flex gap-1 items-center">
					<Icon
						{route}
						link={false}
						express={is_train_route(route, st.trip) && st.trip.data.express}
					/>
					{st.arrival.toLocaleTimeString()}
				</div>
			{/each}
		{:else}
			<div class="text-neutral-400">No transfers available</div>
		{/if}
	</div>
</div>
