<script lang="ts">
	import List from '$lib/components/List.svelte';
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { get_stops, type Stop } from '$lib/api';
	import { pinned_stops } from '$lib/stores';
	import StopPreview from '$lib/components/stop/Preview.svelte';

	let loading_stops = true;

	let stops: Stop[] = [];

	onMount(async () => {
		stops = await get_stops($pinned_stops, true);
		console.log(stops);
		loading_stops = false;

		pinned_stops.subscribe(async (pinned_stops) => {
			stops = await get_stops(pinned_stops, true);
		});
	});

	// maybe in the future use https://melt-ui.com/docs/builders/tooltip for interactive tutorial
</script>

<div class="p-2 text-indigo-200 text-sm">
	<section class="flex flex-col gap-2">
		<!-- <h2 class="text-2xl font-semibold text-indigo-800">Pinned Stops</h2> -->
		<!-- maybe use svelte context module or something else for list stuff -->
		<List title="Pinned Stops" bind:loading={loading_stops}>
			{#if stops.length === 0}
				<div
					transition:slide={{ easing: quintOut, axis: 'y', delay: 100 }}
					class="text-center text-indigo-600 font-medium"
				>
					No stops pinned
				</div>
			{/if}
			{#each stops as stop (stop.id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<StopPreview {stop} />
					<!-- <div role="separator" class="my-2 h-px w-full bg-indigo-600" /> -->
				</div>
			{/each}
		</List>
	</section>
</div>
