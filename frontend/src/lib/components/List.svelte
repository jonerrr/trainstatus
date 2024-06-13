<script lang="ts">
	import { item_heights } from '$lib/stores';
	import { derived } from 'svelte/store';

	let list_el: HTMLDivElement;
	export function scrollIntoView() {
		list_el.scrollIntoView({ behavior: 'smooth' });
	}

	// set a max height for the list
	export let expand: boolean = true;
	export let min_h: number = 50;

	// calculate height of list (maybe make static)
	// const item_heights: number[] = [];
	// $: min_h = item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);

	// const min_h = derived(item_heights, ($item_heights) => {
	// 	const h = $item_heights[id];
	// 	console.log(id, h);
	// 	return h;
	// });

	// export let show_search = false;
	// TODO: figure out how to put min-h logic in here (maybe store each length in a store and get in the list using a list of ids passed to the list)
</script>

<div
	bind:this={list_el}
	style={!expand ? `min-height: ${40 + min_h}px; max-height: ${40 + min_h}px;` : ''}
	class={`relative flex flex-col text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 overflow-auto ${$$props.class} ?? ''}`}
>
	<slot />
</div>
