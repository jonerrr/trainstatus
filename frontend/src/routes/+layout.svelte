<script lang="ts">
	import { onMount, tick, untrack } from 'svelte';

	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';

	import Modal from '$lib/Modal.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import SEO from '$lib/SEO.svelte';
	import type { Source } from '$lib/client';
	import { alert_context, createAlertResource } from '$lib/resources/alerts.svelte';
	import { createPositionResource, position_context } from '$lib/resources/positions.svelte';
	import { createStopTimeResource, stop_time_context } from '$lib/resources/stop_times.svelte';
	import { createTripResource, trip_context } from '$lib/resources/trips.svelte';
	import { current_time } from '$lib/url_params.svelte';

	import '@fontsource/inter';

	import '../app.css';

	let { children } = $props();

	let offline = $state(false);

	// Initialize current_time from URL param on page load
	// If we don't initialize here, the syncing $effect will error out when running replaceState on page load (since router isn't initialized)
	// TODO: maybe add error handling for invalid at param. for example if its a huge number and becomes NaN, the sync $effect runs and errors out
	if (page.data.at) current_time.value = parseInt(page.data.at);
	// TODO: fix clearing time not working

	trip_context.set(
		Object.fromEntries(
			page.data.selected_sources.map((source) => [source, createTripResource(source)])
		)
	);

	stop_time_context.set(
		Object.fromEntries(
			page.data.selected_sources.map((source) => [source, createStopTimeResource(source)])
		)
	);

	position_context.set(
		Object.fromEntries(
			page.data.selected_sources.map((source) => [source, createPositionResource(source)])
		)
	);

	alert_context.set(
		Object.fromEntries(
			page.data.selected_sources.map((source) => [source, createAlertResource(source)])
		)
	);

	// Initialize modal from URL params on page load
	onMount(() => {
		const url = page.url;
		const stop_id = url.searchParams.get('s');
		const route_id = url.searchParams.get('r');
		const trip_id = url.searchParams.get('t');
		const source_id = url.searchParams.get('src') as Source | null;

		if (!source_id) {
			console.warn('No source specified in URL params, skipping modal initialization');
			return;
		}

		if (stop_id) {
			const stop = page.data.stops_by_id[source_id]?.[stop_id];
			if (stop) {
				tick().then(() => replaceState('', { modal: { ...stop, type: 'stop' } }));
			}
		} else if (route_id) {
			const route = page.data.routes_by_id[source_id]?.[route_id];
			if (route) {
				tick().then(() => replaceState('', { modal: { ...route, type: 'route' } }));
			}
		} else if (trip_id) {
			const all_trips_data = trip_context.get();
			const resource = all_trips_data[source_id];
			if (resource) {
				resource.whenReady().then((trips) => {
					const trip = trips.get(trip_id);
					if (trip && page.url.searchParams.get('t') === trip_id) {
						tick().then(() => replaceState('', { modal: { ...trip, type: 'trip' } }));
					}
				});
			}
		}
	});

	// Sync current_time.value with ?at URL param whenever it changes.
	// Reads page.url inside untrack so this effect only re-runs when current_time changes.
	$effect(() => {
		$inspect.trace('Syncing current_time with URL param');
		const val = current_time.value;
		untrack(() => {
			const current_at = page.url.searchParams.get('at');
			const new_at = val !== undefined ? val.toString() : null;
			console.log(
				`current_time changed: ${current_time.value} (URL param at=${current_at}), syncing to ${new_at}`
			);
			if (current_at === new_at) return; // no change needed

			const url = new URL(page.url);
			if (val !== undefined) url.searchParams.set('at', val.toString());
			else url.searchParams.delete('at');
			console.log(`Updating URL search param at=${new_at} (was ${current_at})`);
			replaceState(url.pathname + url.search, page.state);
		});
	});
</script>

<SEO />

<svelte:window ononline={() => (offline = false)} onoffline={() => (offline = true)} />

<!-- Navbar is fixed-position; this wrapper reserves space for it.
     Mobile: pb-16 (bottom bar). Larger screens, md+: pl-20 (left sidebar). -->
<div class="flex h-dvh flex-col pb-16 md:pb-0 md:pl-20">
	<!-- <Header {offline} /> -->
	<main class="relative flex-1 overflow-hidden text-white">
		<Modal />

		{@render children()}
	</main>
</div>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}
</style>
