<script lang="ts">
	import { liveQuery } from 'dexie';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { db } from '$lib/db';
	import List from '$lib/components/List.svelte';
	import StopDialog from '$lib/components/StopDialog.svelte';

	export let stop_ids: string[] = [];
	// export let show_search: boolean = false;

	// let stops = stop_ids.length
	// 	? liveQuery(() => db.stop.where('id').anyOf(stop_ids).toArray())
	// 	: liveQuery(() => db.stop.toArray());
	// $: stop_times = liveQuery(async () => db.stop_time.where('stop_id').equals('250').toArray());

	let stops = liveQuery(() => db.stop.where('id').anyOf(stop_ids).toArray());
</script>

<div class="text-white">
	{#if $stops}
		<List loading={false} class="bg-neutral-800/90 border border-neutral-700 p-1">
			<div slot="header" class="flex self-center mb-2 w-full justify-between">
				<div class="font-semibold text-lg text-indigo-300">Stops</div>
			</div>
			{#each $stops as stop (stop.id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<StopDialog bind:stop />
				</div>
			{/each}
		</List>
	{/if}
</div>
