<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { onMount, tick, type Snippet } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';
	import { current_time, debounce } from '$lib/util.svelte';
	import { SvelteSet } from 'svelte/reactivity';

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	let last_monitored_routes = $state(new SvelteSet<string>());
	let offline = $state(false);
	let is_updating = $state(false);

	let last_at = current_time.value;
	$effect(() => {
		current_time.value;
		debounce(() => {
			if (!current_time.value || last_at === current_time.value) return;
			// console.log('time change, updating rt data');
			try {
				is_updating = true;
				Promise.all([
					trips.update(fetch),
					alerts.update(fetch),
					stop_times.update(fetch, [...monitored_bus_routes], false)
				]);
				last_update = new Date();
				last_st_update = new Date();
			} catch (e) {
				console.error(e);
				offline = true;
			} finally {
				is_updating = false;
			}
		}, 500)();
	});

	onMount(() => {
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

				// update alerts and trips every 15 seconds
				if (now - last_update.getTime() > 1000 * 15) {
					// console.log('Updating rt data');

					await Promise.all([trips.update(fetch), alerts.update(fetch)]);

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
					await stop_times.update(fetch, [...monitored_bus_routes], true);
					// TODO: should we set last_st_update here?
					// last_st_update = new Date();
					last_monitored_routes = new SvelteSet([...monitored_bus_routes]);
					offline = false;
				}
				// update stop times every 15 seconds
				if (update_st) {
					await stop_times.update(fetch, [...monitored_bus_routes], false);
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
			}
		}, 200);

		// $inspect(monitored_bus_routes);

		const id =
			// stop
			page.url.searchParams.get('s') ||
			// route
			page.url.searchParams.get('r') ||
			// trip
			page.url.searchParams.get('t');

		if (id) {
			tick().then(() => {
				// check what type of id it is
				if (id in page.data.routes) {
					pushState('', {
						modal: 'route',
						data: page.data.routes[id]
						// at: at ? parseInt(at) : undefined
					});
				} else if (id in page.data.stops) {
					pushState('', {
						modal: 'stop',
						data: page.data.stops[parseInt(id)]
						// at: at ? parseInt(at) : undefined
					});
				} else if (trips.trips.has(id)) {
					pushState('', {
						modal: 'trip',
						data: trips.trips.get(id)
						// at: at ? parseInt(at) : undefined
					});
				} else {
					// if (at) {
					// 	console.log('pushing state with at', at);
					// 	replaceState(`?at=${at}`, {
					// 		modal: null,
					// 		at: parseInt(at)
					// 	});
					// }
					console.error('Invalid ID', id);
					alert('Invalid ID');
				}
			});
		}

		return () => clearInterval(interval);
	});
</script>

<Header {offline} />
<!--  h-[calc(100dvh-7.5rem)] -->
<main class="max-w-[1000px] relative m-auto tracking-tight">
	<Modal />

	{@render children()}
</main>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}
</style>
