<script lang="ts">
	import Header from '$lib/components/Header.svelte';
	import Navbar from '$lib/components/Navbar.svelte';
	import Toaster from '$lib/components/UndoToaster.svelte';
	import '../app.css';
	import { stops } from '$lib/stores';
	import type { PageData } from './$types';

	export let data: PageData;
	stops.set(data.stops);
</script>

<Toaster />

<Header />

<slot />

<Navbar />

<style lang="postcss">
	:global(body) {
		@apply bg-zinc-900;
		/* @apply bg-gradient-to-r from-gray-900 from-10% via-zinc-900 via-30% to-neutral-900 to-90%; */
	}

	/* :global(.btn) {
		@apply inline-flex items-center justify-center rounded-xl bg-white px-4 py-3
  	font-medium leading-none text-slate-700 shadow hover:opacity-75;
	}

	:global([data-melt-dialog-content] .btn) {
		@apply !shadow-none bg-slate-900 text-white;
	} */
</style>
