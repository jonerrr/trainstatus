<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/stores';
	import { pushState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';

	let { children } = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	let offline = $state(false);
	let is_updating = $state(false);

	onMount(async () => {
		window.addEventListener('offline', (_e) => {
			offline = true;
		});
		window.addEventListener('online', (_e) => {
			offline = false;
		});

		setInterval(async () => {
			if (is_updating) return;
			// console.log('tick');
			try {
				is_updating = true;

				if (new Date().getTime() - last_update.getTime() > 1000 * 15) {
					console.log('Updating rt data');
					// TODO: remove return val from trips/alerts/stop_times.update
					trips.update(fetch);
					alerts.update(fetch);
					await Promise.all([trips.update(fetch), alerts.update(fetch)]);

					offline = false;
					last_update = new Date();
				}

				if (new Date().getTime() - last_st_update.getTime() > 1000 * 15) {
					await stop_times.update(fetch, [...monitored_bus_routes]);
					last_st_update = new Date();
					offline = false;
				}
			} catch (e) {
				console.error(e);
				offline = true;

				// update in 3 seconds
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
		// console.log(id);
		if (id) {
			// check what type of id it is
			if (id in $page.data.routes) {
				await tick();
				pushState('', {
					modal: 'route',
					data: $page.data.routes[id]
				});
			} else if (id in $page.data.stops) {
				await tick();
				pushState('', {
					modal: 'stop',
					data: $page.data.stops[parseInt(id)]
				});
			} else if (trips.trips.has(id)) {
				await tick();
				pushState('', {
					modal: 'trip',
					data: trips.trips.get(id)
				});
			} else {
				console.error('Invalid ID', id);
				alert('Invalid ID');
			}
		}

		// return () => {
		// 	clearInterval(interval);
		// };
	});

	let monitor_delay: number;

	$inspect(monitored_bus_routes);

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
				await stop_times.update(fetch, [...monitored_bus_routes]);
				last_st_update = new Date();
				offline = false;
			} catch (e) {
				console.error(e);
				offline = true;
			}
		}, 50);
	});
</script>

<Header {offline} />
<main class="md:w-[60%] m-auto relative h-[calc(100dvh-7.5rem)]">
	<Modal />

	<!-- {#await data.initial_promise}
		<div class="text-neutral-50 text-4xl flex justify-center">
			<LoaderCircle size="4rem" class="animate-spin" />
		</div>
	{:then _} -->
	{@render children()}
	<!-- {/await} -->
</main>
<Navbar />

<style lang="postcss">
	:global(body) {
		@apply bg-neutral-950;
	}
</style>
