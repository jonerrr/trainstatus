<script lang="ts">
	import '@fontsource/inter';
	import { register } from 'swiper/element/bundle';
	import { onDestroy, onMount } from 'svelte';
	import Header from '$lib/components/Header.svelte';
	import Navbar from '$lib/components/Navbar.svelte';
	import Toaster from '$lib/components/UndoToaster.svelte';
	import '../app.css';
	import { init_data } from '$lib/api_new';
	import { stops } from '$lib/stores';
	import type { PageData } from './$types';

	export let data: PageData;
	stops.set(data.stops);

	let interval: number;

	onMount(() => {
		init_data();

		interval = setInterval(() => {
			init_data();
		}, 10000);
	});

	onDestroy(() => {
		console.log('clearing db intervals');
		clearInterval(interval);
	});

	// register swiper.js for alert carousel
	register();
</script>

<Toaster />

<div class="md:w-[60%] m-auto">
	<Header />

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
