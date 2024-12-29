<script lang="ts">
	// import { slide } from 'svelte/transition';
	import { page } from '$app/state';
	import { type StopTime, stop_times } from '$lib/stop_times.svelte';
	import {
		is_train_route,
		trips,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import { pushState } from '$app/navigation';

	interface TransferProps {
		stop_time: StopTime;
		trip: Trip<BusTripData | TrainTripData>;
		time_format: 'time' | 'countdown';
	}

	const { stop_time, trip, time_format }: TransferProps = $props();

	// get all trips that stop at this stop after this stop time
	const transfers = $derived.by(() => {
		const transfers = [];

		// stop_times.stop_times
		// this should prob be a while loop (or map)
		for (const st of stop_times.stop_times) {
			// if (page.data.train_stops[st.stop_id]?.data.transfers.includes(stop_time.stop_id)) {
			// 	console.log('transfer stop');
			// }
			if (transfers.length >= 3) {
				break;
			}

			if (
				st.trip_id === stop_time.trip_id ||
				(st.stop_id !== stop_time.stop_id &&
					!page.data.train_stops[st.stop_id]?.data.transfers.includes(stop_time.stop_id)) ||
				st.arrival < stop_time.arrival
			) {
				continue;
			}

			const transfer_trip = trips.trips.get(st.trip_id);
			// nobody wants to transfer to the same route
			if (
				!transfer_trip ||
				transfer_trip.route_id === trip.route_id ||
				transfer_trip.direction !== trip.direction
			) {
				continue;
			}

			transfers.push({ ...st, trip: transfer_trip });
		}

		return transfers;
	});
</script>

<div class="flex flex-col px-1 bg-neutral-900 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex gap-2 items-center justify-evenly">
		{#if transfers.length}
			{#each transfers as st}
				{@const route = page.data.routes[st.trip.route_id]}
				<button
					onclick={() => pushState('', { modal: 'trip', data: st.trip })}
					class="transition-colors duration-200 flex rounded bg-neutral-800 shadow-2xl gap-1 p-1 items-center hover:bg-neutral-700 active:bg-neutral-900"
				>
					<Icon
						{route}
						link={false}
						express={is_train_route(route, st.trip) && st.trip.data.express}
					/>
					{#if time_format === 'time'}
						{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
					{:else}
						{((st.arrival.getTime() - new Date().getTime()) / 1000 / 60).toFixed(0)}m
						<!-- {st.arrival - new Date().getTime()} -->
					{/if}
				</button>
			{/each}
		{:else}
			<div class="text-neutral-400">No transfers available</div>
		{/if}
	</div>
</div>
