<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	// import { crossfade, slide } from 'svelte/transition';
	// import { cubicInOut, quintOut } from 'svelte/easing';

	let {
		children,
		class: class_name
	}: {
		children: Snippet<[]>;
		class?: string;
	} = $props();
</script>

<!-- transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }} -->
<div
	class={`flex max-h-[60dvh] flex-col divide-y divide-neutral-800 overflow-auto border-y border-neutral-800 bg-neutral-950 text-base ${class_name ?? ''}`}
>
	{@render children()}
</div>
