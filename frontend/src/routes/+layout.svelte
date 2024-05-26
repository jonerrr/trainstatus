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
</style>
