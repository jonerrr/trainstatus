<script lang="ts">
	// Not used currently
	// import { createEventDispatcher } from 'svelte';

	// const ndispatch = createEventDispatcher();

	let list_el: HTMLDivElement;
	export function scrollIntoView() {
		list_el.scrollIntoView({ behavior: 'smooth' });
	}
	export let show_search = false;
</script>

<div
	bind:this={list_el}
	class={`relative overflow-auto text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 min-h-[30%] ${show_search ? 'max-h-[calc(100vh-11rem)]' : 'max-h-[calc(100vh-8rem)]'} `}
>
	<slot />
</div>
