<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	let list_el: HTMLDivElement;

	let interval: number;

	onMount(() => {
		setInterval(() => {
			if (list_el == null) return;

			const els = Array.from(list_el.querySelectorAll('#list-item'))
				.map((e) => e.clientHeight)
				.sort((a, b) => b - a);
			const largest = els[0];
			// console.log(largest);
			// list_el.querySelectorAll('#list-item').forEach((e) => {
			// 	e.style.height = `${largest}px`;
			// });

			// list_height = els.reduce((h, e) => e.clientHeight + h, 0);
			// list_height += 10;
		}, 100);
	});

	onDestroy(() => {
		clearInterval(interval);
	});
</script>

<div
	bind:this={list_el}
	class="flex flex-col divide-y bg-neutral-900 divide-neutral-800 text-indigo-200 overflow-auto max-h-96 px-2"
>
	<slot />
</div>
