<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	// import { item_heights } from '$lib/stores';
	// import { derived } from 'svelte/store';

	let list_el: HTMLDivElement;
	export function scrollIntoView() {
		list_el.scrollIntoView({ behavior: 'smooth' });
	}

	// set a max height for the list
	export let manage_height: boolean = true;

	let list_height = 0;
	let interval: number;

	if (manage_height) {
		onMount(() => {
			setInterval(() => {
				if (list_el == null) return;
				// TODO: make button component and reuse (it will include list-item id)
				const els = Array.from(list_el.querySelectorAll('#list-item')).slice(0, 3);
				//@ts-expect-error
				list_height = els.reduce((h, e) => e.offsetHeight + h, 0);
				// console.log(list_height);
			}, 50);
		});

		onDestroy(() => {
			clearInterval(interval);
		});
	}
</script>

<div
	bind:this={list_el}
	style={manage_height ? `min-height: ${list_height + 40}px;` : ''}
	class={`relative flex flex-col text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 overflow-auto ${$$props.class} ?? ''}`}
>
	<slot />
</div>
