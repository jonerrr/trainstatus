<script lang="ts">
	import '@fontsource/inter';
	import '../app.css';
	import { register } from 'swiper/element/bundle';
	import { onDestroy, onMount } from 'svelte';
	import { init_data } from '$lib/api';
	import { stops, trips, stop_times, alerts } from '$lib/stores';
	import Header from '$lib/components/Header.svelte';
	import Navbar from '$lib/components/Navbar.svelte';
	import Toaster from '$lib/components/UndoToaster.svelte';
	import Dialog from '$lib/components/Dialog.svelte';
	// import type { PageData } from './$types';

	// export let data: PageData;
	// stops.set(data.stops);
	// alert_store.set(data.stops);
	// trip_store.set(data.trips);
	// stop_time_store.set(data.stopTimes);

	let interval: number;

	onMount(() => {
		// init_data();

		interval = setInterval(() => {
			init_data(fetch, trips, stop_times, alerts);
		}, 10000);
	});

	// Don't think we need this bc its a layout and won't be unmounted
	onDestroy(() => {
		clearInterval(interval);
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
