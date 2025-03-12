<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';
	import { current_time } from '$lib/util.svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let { children, data } = $props();

	let last_update = $state<Date>(new Date());
	// TODO: they need to be sveltesets
	let last_st_update = $state<Date>(new Date());
	// used to check if bus routes have changed
	let last_monitored_routes = $state(new SvelteSet<string>());
	// used to show offline icon in header
	let offline = $state(false);
	// used to prevent multiple updates at the same time
	let is_updating = $state(false);

	let last_at = data.at ? parseInt(data.at) : undefined;

	onMount(() => {
		if (last_at) {
			current_time.value = last_at;
		}
		const id =
			// stop
			page.url.searchParams.get('s') ||
			// route
			page.url.searchParams.get('r') ||
			// trip
			page.url.searchParams.get('t');

		tick().then(() => {
			if (id) {
				// check what type of id it is
				// TODO: should we use replace or push state
				if (id in page.data.routes) {
					replaceState('', {
						modal: 'route',
						data: page.data.routes[id]
					});
				} else if (id in page.data.stops) {
					replaceState('', {
						modal: 'stop',
						data: page.data.stops[parseInt(id)]
					});
				} else if (trips.trips.has(id)) {
					replaceState('', {
						modal: 'trip',
						data: trips.trips.get(id)
					});
				}
			}
		});

		window.addEventListener('offline', (_e) => {
			offline = true;
		});
		window.addEventListener('online', (_e) => {
			offline = false;
		});

		// $inspect(last_monitored_routes);

		const interval = setInterval(async () => {
			// prevent multiple updates at the same time
			if (is_updating) return;

			try {
				is_updating = true;
				const now = new Date().getTime();

				const current_time_changed = last_at !== current_time.value;
				// should update if current time is not set or if it's more than 4 hours old
				const should_update = !current_time.value || current_time.ms >= now - 14400000;
				// update alerts and trips every 15 seconds or when current time changes
				if (should_update && (now - last_update.getTime() > 1000 * 15 || current_time_changed)) {
					// console.log('Updating rt data');

					await Promise.all([
						trips.update(fetch, current_time.value?.toString()),
						alerts.update(fetch, current_time.value?.toString())
					]);

					offline = false;
					last_update = new Date();
				}

				if (monitored_bus_routes.size > 30) {
					// remove until there are 30 left
					const to_remove = Array.from(monitored_bus_routes).slice(0, -30);
					// console.log('removing', to_remove);
					to_remove.forEach((r) => {
						monitored_bus_routes.delete(r);
						last_monitored_routes.delete(r);
					});
				}

				const routes_changed =
					monitored_bus_routes.size !== last_monitored_routes.size ||
					!monitored_bus_routes.isSubsetOf(last_monitored_routes);
				const should_update_st = now - last_st_update.getTime() > 1000 * 15 || current_time_changed;
				if (routes_changed && !should_update_st) {
					// console.log('Updating st bc routes changed');

					// TODO: improve storing bus route so I can update only the new ones
					// Find only the new routes that weren't in last_monitored_routes
					// const new_routes = [...monitored_bus_routes].filter(
					// 	(route) => !last_monitored_routes.has(route)
					// );
					// const new_routes = monitored_bus_routes.difference(last_monitored_routes);

					// if (new_routes.size) {
					// 	// Only update with the new routes
					// 	await stop_times.update(fetch, [...new_routes], true, current_time.value?.toString());

					// 	// const updated_routes = new SvelteSet([...last_monitored_routes]);
					// 	// for (const route of new_routes) {
					// 	// 	updated_routes.add(route);
					// 	// }
					// 	// last_monitored_routes = updated_routes;
					// 	last_monitored_routes = new SvelteSet(last_monitored_routes.union(new_routes));
					// 	offline = false;
					// }
					await stop_times.update(
						fetch,
						[...monitored_bus_routes],
						true,
						current_time.value?.toString()
					);
					last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
					offline = false;
				}
				// update stop times every 15 seconds
				if (should_update && should_update_st) {
					// console.log('Updating st');

					await stop_times.update(
						fetch,
						[...monitored_bus_routes],
						false,
						current_time.value?.toString()
					);
					last_st_update = new Date();
					last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
					offline = false;
				}
			} catch (e) {
				console.error(e);
				offline = true;

				// update in 3 seconds if offline
				last_update = new Date(new Date().getTime() - 1000 * 7);
				last_st_update = new Date(new Date().getTime() - 1000 * 7);
			} finally {
				is_updating = false;
				last_at = current_time.value;
			}
		}, 200);

		// $inspect(monitored_bus_routes);

		return () => clearInterval(interval);
	});
</script>

<Header {offline} />
<!--  h-[calc(100dvh-7.5rem)] -->
<main class="max-w-[1000px] relative m-auto text-white">
	<Modal />

	{@render children()}
</main>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}
</style>
