<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/stores';
	import { pushState } from '$app/navigation';
	import { onMount, setContext, tick, type Snippet } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	let offline = $state(false);
	let is_updating = $state(false);

	// unix timestamp that gets sent to backend
	let current_time = $state<number>();

	// TODO: don't use context bc the initial value is undefined so the list items are broken
	$effect.pre(() => {
		console.log('updating current_time');
		setContext('current_time', current_time);
	});

	onMount(() => {
		const at = $page.url.searchParams.get('at');

		if (at && !isNaN(parseInt(at))) {
			current_time = parseInt(at);
		}

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

					await Promise.all([
						trips.update(fetch, current_time),
						alerts.update(fetch, current_time)
					]);

					offline = false;
					last_update = new Date();
				}

				// update stop times every 15 seconds
				if (now - last_st_update.getTime() > 1000 * 15) {
					await stop_times.update(fetch, [...monitored_bus_routes], false, current_time);
					last_st_update = new Date();
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

		const id =
			// stop
			$page.url.searchParams.get('s') ||
			// route
			$page.url.searchParams.get('r') ||
			// trip
			$page.url.searchParams.get('t');

		if (id) {
			tick().then(() => {
				// check what type of id it is
				if (id in $page.data.routes) {
					pushState('', {
						modal: 'route',
						data: $page.data.routes[id]
					});
				} else if (id in $page.data.stops) {
					pushState('', {
						modal: 'stop',
						data: $page.data.stops[parseInt(id)]
					});
				} else if (trips.trips.has(id)) {
					pushState('', {
						modal: 'trip',
						data: trips.trips.get(id)
					});
				} else {
					console.error('Invalid ID', id);
					alert('Invalid ID');
				}
			});
		}

		return () => clearInterval(interval);
	});

	let monitor_delay: number;

	$inspect(monitored_bus_routes, 'monitored_bus_routes');

	$effect(() => {
		clearTimeout(monitor_delay);
		// need to put offline here so it updates when offline changes
		if (offline) return;
		if (monitored_bus_routes.size > 30) {
			// remove until there are 30 left
			const to_remove = Array.from(monitored_bus_routes).slice(0, -30);
			// console.log('removing', to_remove);
			to_remove.forEach((r) => monitored_bus_routes.delete(r));
		}

		monitor_delay = setTimeout(async () => {
			try {
				if (monitored_bus_routes.size) {
					await stop_times.update(fetch, [...monitored_bus_routes], true, current_time);
					last_st_update = new Date();
					offline = false;
				}
			} catch (e) {
				console.error(e);
				offline = true;
			}
		}, 50);
	});
</script>

<Header bind:current_time {offline} />
<!--  h-[calc(100dvh-7.5rem)] -->
<main class="max-w-[1000px] relative m-auto tracking-tight">
	<Modal {current_time} />

	{@render children()}
</main>
<Navbar {current_time} />

<style lang="postcss">
	:global(body) {
		@apply bg-neutral-900;
	}
</style>
