<script lang="ts">
	import { derived } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { stop_store } from '$lib/api_new';
	import List from '$lib/components/List.svelte';
	import StopDialog from '$lib/components/Stop/Dialog.svelte';

	export let title: string = 'Stops';
	export let stop_ids: string[] = [];
	export let show_search: boolean = false;

	const stops = derived(stop_store, ($stop_store) => {
		const st = $stop_store.filter((st) => stop_ids.includes(st.id));
		return st;
	});
</script>

<div class="text-white">
	{#if $stops}
		<List loading={false} class="bg-neutral-800/90 border border-neutral-700 p-1">
			<div slot="header" class="flex self-center mb-2 w-full justify-between">
				<div class="font-semibold text-lg text-indigo-300">{title}</div>
			</div>
			{#each $stops as stop (stop.id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<StopDialog {stop} />
				</div>
			{/each}
		</List>
	{/if}
</div>
