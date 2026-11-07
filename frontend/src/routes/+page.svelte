<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, CatalogStop, Source } from '#lib/client/index.js';
	import { geolocation } from '#lib/geolocation.svelte.js';
	import List, { type TripItem } from '#lib/List.svelte';
	import { parse_trip_pin, route_pins, stop_pins, trip_pins } from '#lib/pins.svelte.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import {
		calculate_route_height,
		calculate_stop_height,
		calculate_trip_height,
		haversine
	} from '#lib/util.svelte.js';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';

	const transit = getTransitData();
	const current_time = getCurrentTime();

	const catalog_resources = transit.catalogs(() => page.data.selected_sources);
	const catalogs = $derived(catalog_resources.current.filter((r) => r.state === 'ready'));
	const catalogs_loading = $derived(catalog_resources.current.some((c) => c.state === 'loading'));

	function by_source<T>(pick: (source: Source) => T[]) {
		const sources = Object.fromEntries(
			page.data.selected_sources.map((source) => [source, pick(source)])
		) as Record<Source, T[]>;
		return { sources, exists: Object.values(sources).some((items) => items.length > 0) };
	}

	const pinned_trips = $derived(
		by_source((source) =>
			(trip_pins.current[source] ?? []).flatMap((id): TripItem[] => {
				const pin = parse_trip_pin(id);
				return pin ? [{ id, source, ...pin }] : [];
			})
		)
	);
	const pinned_service = transit.trips(() =>
		current_time.value === undefined ? Object.values(pinned_trips.sources).flat() : []
	);
	$effect(() => {
		const pins = trip_pins.current;
		let next = pins;
		for (const read of pinned_service.current) {
			if (read.state !== 'missing') continue;
			const kept = next[read.source].filter((key) => {
				const pin = parse_trip_pin(key);
				return pin?.route_id !== read.route_id || pin.trip_id !== read.trip_id;
			});
			if (kept.length !== next[read.source].length) next = { ...next, [read.source]: kept };
		}
		if (next !== pins) trip_pins.current = next;
	});

	const catalog_of = (source: Source) => catalogs.find((c) => c.source === source)?.value;

	const pinned_stops = $derived(
		by_source((source) =>
			(stop_pins.current[source] ?? [])
				.map((pin) => catalog_of(source)?.stops_by_id.get(pin))
				.filter((s): s is CatalogStop => s !== undefined)
		)
	);

	const pinned_routes = $derived(
		by_source((source) =>
			(route_pins.current[source] ?? [])
				.map((pin) => catalog_of(source)?.routes_by_id.get(pin))
				.filter((r): r is CatalogRoute => r !== undefined)
		)
	);

	// Interface for stops with distance
	interface StopWithDistance extends CatalogStop {
		distance: number;
	}

	const nearby_stops = $derived.by(() => {
		const result = {} as Record<Source, StopWithDistance[]>;

		if (!geolocation.position) return result;
		const current_coords = geolocation.position.coords;

		for (const { source, value: catalog } of catalogs) {
			result[source] = (catalog?.stops ?? [])
				.map((stop) => {
					const distance = haversine(
						current_coords.latitude,
						current_coords.longitude,
						stop.lat,
						stop.lon
					);
					return { ...stop, distance };
				})
				.sort((a, b) => a.distance - b.distance);
		}
		return result;
	});
</script>

{#snippet locate_button()}
	<button
		onclick={() => geolocation.resume()}
		class={[
			'relative flex items-center justify-center rounded-none p-1 shadow-lg transition-colors duration-200',
			{
				'bg-border-default text-primary hover:bg-border-strong active:bg-surface-2':
					geolocation.status === 'loading',
				'bg-accent text-accent-fg hover:bg-accent-hover active:bg-accent':
					geolocation.status === 'active',
				'bg-red-600 text-white hover:bg-red-500 active:bg-red-700': geolocation.status === 'denied'
			}
		]}
		aria-label="Nearby stops"
		title="Nearby stops"
	>
		{#if geolocation.status === 'loading'}
			<Locate class="animate-spin" />
		{:else if geolocation.status === 'denied'}
			<LocateOff />
		{:else}
			<LocateFixed />
		{/if}
	</button>
{/snippet}

<div class="flex h-full flex-col overflow-hidden">
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden">
		{#if pinned_trips.exists}
			<List
				title="Pinned Trips"
				sources={pinned_trips.sources}
				type="trip"
				pins={trip_pins}
				height_calc={calculate_trip_height}
				items_before_scroll={2}
				list_class="max-h-[25dvh]"
			/>
		{/if}

		{#if pinned_routes.exists}
			<List
				title="Pinned Routes"
				pins={route_pins}
				sources={pinned_routes.sources}
				type="route"
				height_calc={calculate_route_height}
				items_before_scroll={2}
				list_class="max-h-[25dvh]"
			/>
		{/if}

		<!-- TODO: loading pinned stops in SSR always puts the 1 as the first -->
		{#if pinned_stops.exists}
			<List
				title="Pinned stops"
				pins={stop_pins}
				sources={pinned_stops.sources}
				type="stop"
				height_calc={calculate_stop_height}
				items_before_scroll={2}
				ssr_min={0}
				list_class="max-h-[25dvh]"
			/>
		{/if}
	</div>

	<div class="min-h-0 flex-1">
		<!-- TODO: either hide or show error message when nearby_stops is empty and location perms were denied -->
		<!-- maybe put it inside of List, since we should also start showing an error message when the stop search returns empty -->
		<!-- {#if nearby_stops} -->
		<List
			title="Nearby Stops"
			type="stop"
			container_class="h-full"
			sources={nearby_stops}
			pins={stop_pins}
			header_slot={locate_button}
			height_calc={calculate_stop_height}
			loading={catalogs_loading || (geolocation.status === 'loading' && !geolocation.position)}
		/>
		<!-- {:else}
			<p class="p-4 text-sm text-muted">
				Unable to get nearby stops. Please allow location access and refresh the page.
			</p>
		{/if} -->
	</div>
</div>
