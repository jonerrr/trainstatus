<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { calculate_stop_height } from '$lib/static';
	import { haversine } from '$lib/util.svelte';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';
	import type { Source, Stop } from '@trainstatus/client';
	import { PersistedState, useGeolocation } from 'runed';

	// Geolocation state
	const location_status = new PersistedState<'unknown' | 'loading' | 'granted' | 'denied'>(
		'location_status',
		'unknown'
	);
	const location = useGeolocation({
		immediate: ['unknown', 'granted'].includes(location_status.current)
	});

	const isLoading = $derived(
		location.position.coords.latitude === Number.POSITIVE_INFINITY && location.error === null
	);

	// Update location status based on geolocation state
	$effect(() => {
		if (location.error) {
			location_status.current = 'denied';
		} else if (location.position.coords.latitude !== Number.POSITIVE_INFINITY && !location.error) {
			location_status.current = 'granted';
		}
	});

	// Interface for stops with distance
	interface StopWithDistance extends Stop {
		distance: number;
	}

	interface SourceData {
		source: Source;
		data: StopWithDistance[];
	}

	// Compute nearby stops sorted by distance for each source
	const nearby_stops = $derived.by((): SourceData[] => {
		// If we have location, sort stops by distance
		if (!isLoading && !location.error) {
			return page.data.stops.map((source) => {
				const data = source.data
					.map((stop: Stop) => {
						const distance = haversine(
							location.position.coords.latitude,
							location.position.coords.longitude,
							stop.geom.coordinates.Point.y,
							stop.geom.coordinates.Point.x
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance);
				return { source: source.source, data };
			});
		}
		// Return unsorted data if no location
		return page.data.stops.map((source) => ({
			source: source.source,
			data: source.data.map((stop: Stop) => ({ ...stop, distance: Infinity }))
		}));
	});

	// Height calculation
	let list_height = $state(0);
	let pin_list_height = $state(0);
	const nearby_list_height = $derived(list_height - pin_list_height);
</script>

{#snippet locate_button()}
	<button
		onclick={() => location.resume()}
		class={{
			'locate-button': true,
			'locate-active': location_status.current === 'granted',
			'locate-denied': location_status.current === 'denied',
			'locate-inactive':
				location_status.current === 'unknown' || location_status.current === 'loading'
		}}
		aria-label="Nearby stops"
		title="Nearby stops"
	>
		{#if location_status.current === 'denied'}
			<LocateOff />
		{:else if location_status.current === 'granted'}
			<LocateFixed />
		{:else if isLoading}
			<Locate class="animate-spin" />
		{:else}
			<Locate />
		{/if}
	</button>
{/snippet}

<!--  overflow-hidden -->
<div class="flex max-h-[calc(100dvh-10.5rem)] flex-col" bind:offsetHeight={list_height}>
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden" bind:offsetHeight={pin_list_height}>
		<!-- Pinned items can be added here later -->
	</div>

	<div>
		<List
			title="Nearby Stops"
			type="stop"
			style="max-height: {nearby_list_height}px"
			sources={nearby_stops}
			pins={stop_pins}
			header_slot={locate_button}
			height_calc={calculate_stop_height}
		/>
	</div>
</div>

<style>
	@reference "../app.css";

	.locate-button {
		@apply relative flex items-center justify-center rounded-lg p-1 shadow-lg transition-all duration-200;
	}

	.locate-active {
		@apply bg-blue-600 text-white;
	}

	.locate-denied {
		@apply bg-red-600 text-white hover:bg-red-500 active:bg-red-700;
	}

	.locate-inactive {
		@apply bg-neutral-700 text-white hover:bg-neutral-600 active:bg-neutral-800;
	}
</style>
