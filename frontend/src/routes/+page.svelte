<script lang="ts">
	import { Locate, LocateFixed, LocateOff } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import {
		pinned_stops,
		pinned_routes,
		location_status,
		LocationStatus,
		stops,
		bus_mode,
		bus_stops,
		item_heights
	} from '$lib/stores';
	import StopList from '$lib/components/Stop/List.svelte';
	import RouteAlertList from '$lib/components/RouteAlert/List.svelte';

	let stop_ids: string[] = [];

	//  from https://www.geeksforgeeks.org/haversine-formula-to-find-distance-between-two-points-on-a-sphere/
	function haversine(lat1: number, lon1: number, lat2: number, lon2: number) {
		// distance between latitudes
		// and longitudes
		let dLat = ((lat2 - lat1) * Math.PI) / 180.0;
		let dLon = ((lon2 - lon1) * Math.PI) / 180.0;

		// convert to radiansa
		lat1 = (lat1 * Math.PI) / 180.0;
		lat2 = (lat2 * Math.PI) / 180.0;

		// apply formulae
		let a =
			Math.pow(Math.sin(dLat / 2), 2) +
			Math.pow(Math.sin(dLon / 2), 2) * Math.cos(lat1) * Math.cos(lat2);
		let rad = 6371;
		let c = 2 * Math.asin(Math.sqrt(a));
		return rad * c;
	}

	async function get_nearby_stops() {
		location_status.set(LocationStatus.Loading);
		navigator.geolocation.getCurrentPosition(
			async (position) => {
				// TODO: make sure bus list has loaded before getting location (call get_nearby_stops after loading)
				const { coords } = position;
				const stop_list = $bus_mode ? [...$stops, ...$bus_stops] : $stops;
				const closest_stops = stop_list
					.map((stop) => {
						const distance = haversine(coords.latitude, coords.longitude, stop.lat, stop.lon);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance)
					.slice(0, 15);
				console.log(closest_stops);
				// const b_sids = closest_stops.filter((s) => typeof s.id === 'number');
				// console.log(b_sids);
				stop_ids = closest_stops.filter((s) => typeof s.id === 'string').map((stop) => stop.id);
				location_status.set(LocationStatus.Granted);
			},
			(e) => {
				console.error('Error getting location', e);

				location_status.set(LocationStatus.Denied);
			}
		);
	}

	onMount(() => {
		if ($location_status === LocationStatus.Granted) {
			get_nearby_stops();
		} else if ($location_status === LocationStatus.Loading) {
			// reset location status if stuck loading
			location_status.set(LocationStatus.NeverAsked);
		}
	});

	// maybe in the future use https://melt-ui.com/docs/builders/tooltip for interactive tutorial
</script>

<svelte:head>
	<title>TrainStat.us | Home</title>
	<!-- TODO: show rt delays in embed -->
</svelte:head>

<div class="p-1 text-indigo-200 text-sm flex flex-col gap-2 h-[calc(100vh-8rem)]">
	{#if $pinned_routes.length}
		<RouteAlertList expand={false} title="Pinned Routes" bind:route_ids={$pinned_routes} />
	{/if}

	{#if $pinned_stops.length}
		<StopList expand={false} bind:stop_ids={$pinned_stops} title="Pinned Stops" />
	{/if}

	<!-- closest stops -->
	<StopList bind:stop_ids title="Nearby Stops" show_location={true}>
		<div slot="location" class="flex gap-2">
			{#if $location_status === LocationStatus.Loading}
				<div class="flex gap-1 items-center text-white rounded px-2 py-1 bg-indigo-600">
					<Locate class="w-4 h-4 animate-spin" />
				</div>
			{:else}
				<button
					aria-label="Nearby stops"
					class="items-center bg-indigo-500 text-white rounded px-2 py-1 active:bg-indigo-600 hover:bg-indigo-600"
					on:click={get_nearby_stops}
				>
					{#if $location_status === LocationStatus.Denied}
						<LocateOff class="w-4 h-4" />
					{:else if $location_status === LocationStatus.Granted}
						<LocateFixed class="w-4 h-4" />
					{:else}
						<Locate class="w-4 h-4" />
					{/if}
				</button>
			{/if}
		</div>
	</StopList>
</div>
