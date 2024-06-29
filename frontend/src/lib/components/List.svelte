<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	// import { item_heights } from '$lib/stores';
	// import { derived } from 'svelte/store';

	let list_el: HTMLDivElement;
	export function scrollIntoView() {
		list_el.scrollIntoView({ behavior: 'smooth' });
	}

	// manage the min/max height of the list
	export let manage_height: boolean = true;

	let list_height = 0;
	let interval: number;

	if (manage_height) {
		onMount(() => {
			setInterval(() => {
				if (list_el == null) return;
				// TODO: make button component and reuse (it will include list-item id)
				const els = Array.from(list_el.querySelectorAll('#list-item')).slice(0, 3);
				// console.log(els.length);
				list_height = els.reduce((h, e) => e.clientHeight + h, 0);
				list_height += 10;
			}, 50);
		});

		onDestroy(() => {
			clearInterval(interval);
		});
	}
</script>

<div
	bind:this={list_el}
	style={manage_height ? `min-height: ${list_height}px; max-height: ${list_height}px;` : ''}
	class={`relative flex flex-col text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 overflow-auto ${$$props.class} ?? ''}`}
>
	<slot />
</div>
