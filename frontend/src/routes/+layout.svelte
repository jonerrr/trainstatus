<script lang="ts">
	import { onMount, tick, untrack } from 'svelte';

	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';

	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import SEO from '$lib/SEO.svelte';
	import { alert_context, createAlertResource } from '$lib/resources/alerts.svelte';
	import { default_sources } from '$lib/resources/index.svelte';
	import { createPositionResource, position_context } from '$lib/resources/positions.svelte';
	import { createStopTimeResource, stop_time_context } from '$lib/resources/stop_times.svelte';
	import { createTripResource, trip_context } from '$lib/resources/trips.svelte';
	import { current_time } from '$lib/url_params.svelte';

	import '@fontsource/inter';

	import '../app.css';

	// Trigger a backward view transition when the browser back button fires a
	// popstate event (shallow-routing popstate is not caught by onnavigate).
	// Svelte 5 batches DOM updates in a microtask, so at the point our listener
	// runs the DOM still shows the old state — startViewTransition captures it
	// as "old", tick() flushes the pending re-render as "new".
	// Guard: only animate when the modal dialog is currently open in the DOM so
	// we don't suppress the root transition for regular back-button page navs.
	// TODO: fix browser back and forward buttons causing transition to run twice and look bad
	function handle_popstate() {
		if (typeof document === 'undefined' || !document.startViewTransition) return;
		if (!document.querySelector('dialog[open]')) return;
		document.documentElement.dataset.modalDirection = 'backward';
		document.startViewTransition(tick).finished.finally(() => {
			delete document.documentElement.dataset.modalDirection;
		});
	}

	let { children } = $props();

	let offline = $state(false);

	// Initialize current_time from URL param on page load
	// If we don't initialize here, the syncing $effect will error out when running replaceState on page load (since router isn't initialized)
	if (page.data.at) current_time.value = parseInt(page.data.at);
	// TODO: fix clearing time not working

	// TODO: handle offline from new fetching method
	const { initial_trips, initial_stop_times, initial_positions, initial_alerts } = page.data;
	trip_context.set(
		Object.fromEntries(
			initial_trips.map(({ source, data }) => [source, createTripResource(source, data)])
		) as any
	);

	stop_time_context.set(
		Object.fromEntries(
			initial_stop_times.map(({ source, data }) => [source, createStopTimeResource(source, data)])
		) as any
	);

	position_context.set(
		Object.fromEntries(
			initial_positions.map(({ source, data }) => [source, createPositionResource(source, data)])
		) as any
	);

	alert_context.set(
		Object.fromEntries(
			initial_alerts.map(({ source, data }) => [source, createAlertResource(source, data)])
		) as any
	);

	// Initialize modal from URL params on page load
	onMount(() => {
		const url = page.url;
		// TODO: maybe include the source in the URL params so we don't have to loop over all of them to check
		const stop_id = url.searchParams.get('s');
		const route_id = url.searchParams.get('r');
		const trip_id = url.searchParams.get('t');

		if (stop_id) {
			for (const source of default_sources) {
				const stop = page.data.stops_by_id[source]?.[stop_id];
				if (stop) {
					tick().then(() => replaceState('', { modal: { ...stop, type: 'stop' } }));
					break;
				}
			}
		} else if (route_id) {
			for (const source of default_sources) {
				const route = page.data.routes_by_id[source]?.[route_id];
				if (route) {
					tick().then(() => replaceState('', { modal: { ...route, type: 'route' } }));
					break;
				}
			}
		} else if (trip_id) {
			const all_trips = trip_context.get();
			if (all_trips) {
				for (const source of default_sources) {
					const trip = all_trips[source]?.value?.get(trip_id);
					if (trip) {
						tick().then(() => replaceState('', { modal: { ...trip, type: 'trip' } }));
						break;
					}
				}
			}
		}
	});

	// Sync current_time.value → ?at URL param whenever it changes.
	// Reads page.url inside untrack so this effect only re-runs when current_time changes.
	$effect(() => {
		const val = current_time.value;
		untrack(() => {
			const current_at = page.url.searchParams.get('at');
			const new_at = val !== undefined ? val.toString() : null;
			if (current_at === new_at) return; // no change needed

			const url = new URL(page.url);
			if (val !== undefined) url.searchParams.set('at', val.toString());
			else url.searchParams.delete('at');
			replaceState(url.pathname + url.search, page.state);
		});
	});
</script>

<SEO />

<svelte:window
	ononline={() => (offline = false)}
	onoffline={() => (offline = true)}
	onpopstate={handle_popstate}
/>

<!-- Navbar is fixed-position; this wrapper reserves space for it.
     Mobile: pb-16 (bottom bar). Desktop lg+: pl-20 (left sidebar). -->
<div class="flex h-dvh flex-col pb-16 lg:pb-0 lg:pl-20">
	<Header {offline} />
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
