<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { derived, writable } from 'svelte/store';
	import { all_route_ids } from '$lib/api';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';
	export let expand: boolean = true;
	// export let accordion: boolean = false;

	if (!route_ids.length) route_ids = all_route_ids;

	// calculate height of list
	// let item_heights: number[] = [];
	const item_heights = writable<number[]>([]);
	// $: {
	// 	if (route_ids.length < item_heights.length) {
	// 		// An item was removed, update item_heights
	// 		item_heights = item_heights.slice(0, route_ids.length);
	// 	}
	// }
	// const min_h = derived(item_heights, ($item_heights) => {
	// 	const h = $item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);
	// 	if (h < 30) return 30;
	// 	else return h;
	// });
	// $: console.log($min_h);

	// $: min_h = item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);
</script>

<!-- TODO: allow user to customize list length or add buttom to add /subtract shown pinned routes -->
<!-- <div
	style={!expand ? `min-height: ${40 + $min_h}px; max-height: ${40 + $min_h}px;` : ''}
	class={`overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[calc(100dvh-8rem)]`}
> -->
<List bind:expand class="max-h-[calc(100dvh-8rem)]">
	<div class="flex text-lg justify-between self-center w-full">
		<div class="font-semibold text-indigo-300">{title}</div>
	</div>
	<div class="flex flex-col mt-1 gap-1">
		{#each route_ids as route_id, i (route_id)}
			<div
				bind:offsetHeight={$item_heights[i]}
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1"
				transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
			>
				<Trigger {route_id} />
			</div>
		{/each}
	</div>
</List>
<!-- </div> -->
