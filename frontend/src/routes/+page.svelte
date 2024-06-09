<script lang="ts">
	import { Locate, LocateFixed, LocateOff } from 'lucide-svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { onMount } from 'svelte';
	import { pinned_stops, pinned_routes, location_status, LocationStatus, stops } from '$lib/stores';
	import StopList from '$lib/components/Stop/List.svelte';
	import RouteAlertList from '$lib/components/RouteAlert/List.svelte';

	let stop_ids: string[] = [];

	async function get_nearby_stops() {
		location_status.set(LocationStatus.Loading);
		navigator.geolocation.watchPosition(
			async (position) => {
				const { coords } = position;

				const closest_stops = $stops
					.map((stop) => {
						const distance = Math.sqrt(
							Math.pow(stop.lat - coords.latitude, 2) + Math.pow(stop.lon - coords.longitude, 2)
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance)
					.slice(0, 15);
				stop_ids = closest_stops.map((stop) => stop.id);
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

<!-- TODO: URL based routing -->
<!-- {#if $page.url.searchParams.has('s')} -->
<!-- <StopDialog stop={$stops.find((s) => s.id === $page.url.searchParams.get('s'))} /> -->
<!-- {/if} -->

<div class="p-1 text-indigo-200 text-sm flex flex-col gap-2 h-[calc(100vh-8rem)]">
	{#if $pinned_routes.length}
		<RouteAlertList title="Pinned Routes" bind:route_ids={$pinned_routes} />
		<!-- {:else}
		<div
			transition:slide={{ easing: quintOut, axis: 'y', delay: 100 }}
			class="text-center text-indigo-500 font-semibold text-lg"
		>
			No routes pinned
		</div> -->
	{/if}

	<!-- pinned stop list -->
	{#if $pinned_stops.length}
		<StopList bind:stop_ids={$pinned_stops} title="Pinned Stops" />
		<!-- {:else}
		<div
			transition:slide={{ easing: quintOut, axis: 'y', delay: 100 }}
			class="text-center text-indigo-500 font-semibold text-lg"
		>
			No stops pinned
		</div> -->
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
