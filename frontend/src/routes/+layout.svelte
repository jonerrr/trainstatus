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
	import { current_time, debounce } from '$lib/util.svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let { children, data } = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	// used to check if bus routes have changed
	let last_monitored_routes = $state(new SvelteSet<string>());
	// used to show offline icon in header
	let offline = $state(false);
	// used to prevent multiple updates at the same time
	let is_updating = $state(false);

	let last_at = data.at ? parseInt(data.at) : undefined;
	// $effect(() => {
	// 	current_time.value;
	// 	if (last_at) console.log('updating time');
	// if (page.state.at && page.state.at !== last_at) {
	// 	console.log('updating trips bc at change', page.state.at);
	// 	last_at = page.state.at;
	// 	debounce(() => {
	// 		try {
	// 			is_updating = true;
	// 			Promise.all([
	// 				trips.update(fetch, page.state.at),
	// 				alerts.update(fetch, page.state.at),
	// 				stop_times.update(fetch, [...monitored_bus_routes], false, page.state.at)
	// 			]);
	// 			last_update = new Date();
	// 			last_st_update = new Date();
	// 		} catch (e) {
	// 			console.error(e);
	// 			offline = true;
	// 		} finally {
	// 			is_updating = false;
	// 		}
	// 	}, 500)();
	// }
	// });

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

		const interval = setInterval(async () => {
			// prevent multiple updates at the same time
			if (is_updating) return;

			try {
				is_updating = true;
				const now = new Date().getTime();

				// update alerts and trips every 15 seconds or when curren time changes
				if (now - last_update.getTime() > 1000 * 15 || last_at !== current_time.value) {
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
					to_remove.forEach((r) => monitored_bus_routes.delete(r));
				}

				const new_routes =
					monitored_bus_routes.size !== last_monitored_routes.size ||
					!monitored_bus_routes.isSubsetOf(last_monitored_routes);
				const update_st = now - last_st_update.getTime() > 1000 * 15;
				if (new_routes && !update_st) {
					// if monitored routes have changed, update stop times
					await stop_times.update(
						fetch,
						[...monitored_bus_routes],
						true,
						current_time.value?.toString()
					);
					// TODO: should we set last_st_update here?
					// last_st_update = new Date();
					last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
					offline = false;
				}
				// update stop times every 15 seconds
				if (update_st || last_at !== current_time.value) {
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
<main class="max-w-[1000px] relative m-auto">
	<Modal />

	{@render children()}
</main>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}
</style>
