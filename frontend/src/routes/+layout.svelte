<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { register } from 'swiper/element/bundle';
	import { onDestroy, onMount } from 'svelte';
	import { update_data } from '$lib/api';
	import { update_bus_data } from '$lib/bus_api';
	import {
		trips,
		stop_times,
		alerts,
		monitored_routes,
		bus_trips,
		bus_stop_times,
		data_at
	} from '$lib/stores';
	import Header from '$lib/components/Header.svelte';
	import Navbar from '$lib/components/Navbar.svelte';
	import Dialog from '$lib/components/Dialog.svelte';

	let interval: number;
	let bus_interval: number;

	let last_monitored_routes: string[] = [];

	onMount(async () => {
		// convert time to unix timestamp
		const time = $data_at ? Math.floor($data_at.getTime() / 1000) : null;

		// update subway data every 10 sec only if they want realtime data
		if (!time) {
			interval = setInterval(async () => {
				await update_data(fetch, trips, stop_times, alerts, null);
			}, 10000);

			// Interval for update_bus_data
			bus_interval = setInterval(async () => {
				//TODO: maybe add a check to make sure length is greater than 0
				await update_bus_data(fetch, bus_trips, bus_stop_times, last_monitored_routes, null);
			}, 30000);
		} else {
			await update_data(fetch, trips, stop_times, alerts, time);
			await update_bus_data(fetch, bus_trips, bus_stop_times, last_monitored_routes, time);
		}

		// update bus routes data when monitored routes change
		monitored_routes.subscribe(async (routes) => {
			if (routes.length && routes.sort().toString() !== last_monitored_routes.sort().toString()) {
				// console.log('updating bus data', routes);
				last_monitored_routes = routes.sort();
				await update_bus_data(fetch, bus_trips, bus_stop_times, routes, time);
			}
		});
	});

	// Don't think we need this bc its a layout and won't be unmounted
	onDestroy(() => {
		clearInterval(interval);
		clearInterval(bus_interval);
	});

	// preserve at query string
	// beforeNavigate(({ from, to, cancel }) => {
	// 	if (from?.url.searchParams.has('at') && !to?.url.searchParams.has('at')) {
	// 		cancel();
	// 		goto(to?.url.pathname + `?at=${from.url.searchParams.get('at')}`);
	// 	}
	// });

	// register swiper.js for alert carousel
	register();
</script>

<svelte:head>
	<meta
		property="og:description"
		content="The best website to see view MTA subway (and bus) times and alerts."
	/>
</svelte:head>

<div class="md:w-[60%] m-auto">
	<Header />

	<Dialog />

	<slot />
</div>
<Navbar />

<style lang="postcss">
	:global(body) {
		@apply bg-neutral-900;
	}

	/* :global(.btn) {
		@apply inline-flex items-center justify-center rounded-xl bg-white px-4 py-3
  	font-medium leading-none text-slate-700 shadow hover:opacity-75;
	}

	:global([data-melt-dialog-content] .btn) {
		@apply !shadow-none bg-slate-900 text-white;
	} */
</style>
