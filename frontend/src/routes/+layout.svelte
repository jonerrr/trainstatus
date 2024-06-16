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
		bus_stop_times
	} from '$lib/stores';
	import Header from '$lib/components/Header.svelte';
	import Navbar from '$lib/components/Navbar.svelte';
	import Toaster from '$lib/components/UndoToaster.svelte';
	import Dialog from '$lib/components/Dialog.svelte';

	let interval: number;
	let bus_interval: number;

	let last_monitored_routes: string[] = [];

	onMount(async () => {
		interval = setInterval(async () => {
			await update_data(fetch, trips, stop_times, alerts);
		}, 15000);

		monitored_routes.subscribe(async (routes) => {
			console.log(routes);
			// if (JSON.stringify(routes) !== JSON.stringify(last_monitored_routes)) {
			// 	last_monitored_routes = routes;
			// 	console.log(routes);
			// 	// await update_bus_data(fetch, bus_trips, bus_stop_times, routes);
			// }
		});

		// Interval for update_bus_data
		// bus_interval = setInterval(async () => {
		// 	await update_bus_data(fetch, bus_trips, bus_stop_times, last_monitored_routes);
		// }, 40000);
	});

	// Don't think we need this bc its a layout and won't be unmounted
	onDestroy(() => {
		clearInterval(interval);
		clearInterval(bus_interval);
	});

	// register swiper.js for alert carousel
	register();
</script>

<Toaster />

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
