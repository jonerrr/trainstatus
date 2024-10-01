<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';

	export let state: App.PageState;
</script>

<button
	id="list-item"
	class="w-full flex justify-between items-center py-1 px-1 hover:bg-neutral-950"
	transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
	on:click={() => {
		pushState('', state);
	}}
>
	<slot />
</button>
