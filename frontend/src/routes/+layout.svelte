<script lang="ts">
	import { page } from '$app/state';

	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import { alert_context, createAlertResource } from '$lib/resources/alerts.svelte';
	import { createPositionResource, position_context } from '$lib/resources/positions.svelte';
	import { createStopTimeResource, stop_time_context } from '$lib/resources/stop_times.svelte';
	import { createTripResource, trip_context } from '$lib/resources/trips.svelte';

	import '@fontsource/inter';

	import '../app.css';

	let { children } = $props();

	let offline = $state(false);

	const params = {};
	// TODO: handle offline from new fetching method
	const { initial_trips, initial_stop_times, initial_positions, initial_alerts } = page.data;

	trip_context.set(
		Object.fromEntries(
			initial_trips.map(({ source, data }) => [source, createTripResource(source, params, data)])
		) as any
	);

	stop_time_context.set(
		Object.fromEntries(
			initial_stop_times.map(({ source, data }) => [
				source,
				createStopTimeResource(source, params, data)
			])
		) as any
	);

	position_context.set(
		Object.fromEntries(
			initial_positions.map(({ source, data }) => [
				source,
				createPositionResource(source, params, data)
			])
		) as any
	);

	alert_context.set(
		Object.fromEntries(
			initial_alerts.map(({ source, data }) => [source, createAlertResource(source, params, data)])
		) as any
	);

	// TODO: remove old code below once sharing and offline functionality is working
	// let last_update = $state<Date>(new Date());
	// let last_st_update = $state<Date>(new Date());
	// // used to check if bus routes have changed
	// let last_monitored_routes = $state(new SvelteSet<string>());
	// // used to show offline icon in header
	// // used to prevent multiple updates at the same time
	// let is_updating = $state(false);

	// let last_at = data.at ? parseInt(data.at) : undefined;

	// onMount(() => {
	// 	if (last_at) {
	// 		current_time.value = last_at;
	// 	}
	// 	const id =
	// 		// stop
	// 		page.url.searchParams.get('s') ||
	// 		// route
	// 		page.url.searchParams.get('r') ||
	// 		// trip
	// 		page.url.searchParams.get('t');

	// 	tick().then(() => {
	// 		if (id) {
	// 			// check what type of id it is
	// 			// TODO: should we use replace or push state
	// 			if (id in page.data.routes) {
	// 				replaceState('', {
	// 					modal: 'route',
	// 					data: page.data.routes[id]
	// 				});
	// 			} else if (id in page.data.stops) {
	// 				replaceState('', {
	// 					modal: 'stop',
	// 					data: page.data.stops[parseInt(id)]
	// 				});
	// 			} else if (trips.trips.has(id)) {
	// 				replaceState('', {
	// 					modal: 'trip',
	// 					data: trips.trips.get(id)
	// 				});
	// 			}
	// 		}
	// 	});

	// const finished = $derived(page.url.pathname.startsWith('/charts'));

	// $inspect(last_monitored_routes);

	// 	const interval = setInterval(async () => {
	// 		// prevent multiple updates at the same time
	// 		if (is_updating) return;

	// 		try {
	// 			is_updating = true;
	// 			const now = new Date().getTime();

	// 			const current_time_changed = last_at !== current_time.value;
	// 			// should update if current time is not set or if it's more than 4 hours old
	// 			const should_update = !current_time.value || current_time.ms >= now - 14400000;
	// 			// update alerts and trips every 15 seconds or when current time changes
	// 			if (current_time_changed || (should_update && now - last_update.getTime() > 1000 * 15)) {
	// 				// console.log('Updating rt data');

	// 				await Promise.all([
	// 					trips.update(fetch, current_time.value?.toString()),
	// 					alerts.update(fetch, current_time.value?.toString())
	// 				]);

	// 				offline = false;
	// 				last_update = new Date();
	// 			}

	// 			if (monitored_bus_routes.size > 30) {
	// 				// remove until there are 30 left
	// 				const to_remove = Array.from(monitored_bus_routes).slice(0, -30);
	// 				// console.log('removing', to_remove);
	// 				to_remove.forEach((r) => {
	// 					monitored_bus_routes.delete(r);
	// 					last_monitored_routes.delete(r);
	// 				});
	// 			}

	// 			const routes_changed =
	// 				monitored_bus_routes.size !== last_monitored_routes.size ||
	// 				!monitored_bus_routes.isSubsetOf(last_monitored_routes);
	// 			const should_update_st = now - last_st_update.getTime() > 1000 * 15;
	// 			if (routes_changed && !should_update_st) {
	// 				// console.log('Updating st bc routes changed');

	// 				// TODO: improve storing bus route so I can update only the new ones
	// 				// Find only the new routes that weren't in last_monitored_routes
	// 				// const new_routes = [...monitored_bus_routes].filter(
	// 				// 	(route) => !last_monitored_routes.has(route)
	// 				// );
	// 				// const new_routes = monitored_bus_routes.difference(last_monitored_routes);

	// 				// if (new_routes.size) {
	// 				// 	// Only update with the new routes
	// 				// 	await stop_times.update(fetch, [...new_routes], true, current_time.value?.toString());

	// 				// 	// const updated_routes = new SvelteSet([...last_monitored_routes]);
	// 				// 	// for (const route of new_routes) {
	// 				// 	// 	updated_routes.add(route);
	// 				// 	// }
	// 				// 	// last_monitored_routes = updated_routes;
	// 				// 	last_monitored_routes = new SvelteSet(last_monitored_routes.union(new_routes));
	// 				// 	offline = false;
	// 				// }
	// 				await stop_times.update(
	// 					fetch,
	// 					[...monitored_bus_routes],
	// 					true,
	// 					current_time.value?.toString()
	// 				);
	// 				last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
	// 				offline = false;
	// 			}
	// 			// update stop times every 15 seconds
	// 			if (current_time_changed || (should_update && should_update_st)) {
	// 				// console.log('Updating st');

	// 				await stop_times.update(
	// 					fetch,
	// 					[...monitored_bus_routes],
	// 					false,
	// 					current_time.value?.toString()
	// 				);
	// 				last_st_update = new Date();
	// 				last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
	// 				offline = false;
	// 			}
	// 		} catch (e) {
	// 			console.error(e);
	// 			offline = true;

	// 			// update in 3 seconds if offline
	// 			last_update = new Date(new Date().getTime() - 1000 * 7);
	// 			last_st_update = new Date(new Date().getTime() - 1000 * 7);
	// 		} finally {
	// 			is_updating = false;
	// 			last_at = current_time.value;
	// 		}
	// 	}, 200);

	// 	// $inspect(monitored_bus_routes);

	// 	return () => clearInterval(interval);
	// });

	// $effect(() => {
	// 	current_time.value;
	// 	tick().then(() => {
	// 		const url = new URL(window.location.href);

	// 		// use existing url because we don't want to lose other query params
	// 		if (current_time.value) {
	// 			url.searchParams.set('at', current_time.value.toString());
	// 		} else {
	// 			url.searchParams.delete('at');
	// 		}

	// 		// only update url if it has changed
	// 		const new_url = url.toString();
	// 		if (new_url !== window.location.href) {
	// 			replaceState(new_url, {
	// 				modal: 'settings'
	// 			});
	// 		}
	// 	});
	// });

	const url_mappings = {
		'/': 'Home',
		'/stops': 'Stops',
		'/routes': 'Routes',
		'/alerts': 'Alerts',
		'/charts': 'Charts'
	} as const;

	const title = $derived.by(() => {
		switch (page.state.modal?.type) {
			case 'stop':
				return `Stop: ${page.state.modal.name}`;
			case 'route':
				return `Route: ${page.state.modal.short_name}`;
			case 'trip':
				return `${page.state.modal.route_id} Trip`;
			// TODO: settings
		}

		const title =
			page.url.pathname in url_mappings
				? url_mappings[page.url.pathname as keyof typeof url_mappings]
				: '???'; // shouldn't happen since we don't have any other routes, but just in case

		return `${title} | Train Status`;
	});
</script>

<svelte:head>
	<title>{title}</title>
</svelte:head>

<svelte:window ononline={() => (offline = false)} onoffline={() => (offline = true)} />

<Header {offline} />
<!--  h-[calc(100dvh-7.5rem)] -->
<main class="relative m-auto text-white">
	<Modal />

	{@render children()}
</main>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}
</style>
