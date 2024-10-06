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
	class={`flex flex-col max-h-[60dvh] border-y border-neutral-800 divide-y overflow-auto overscroll-none divide-neutral-800 text-base ${class_name ?? ''}`}
>
	{@render children()}
</div>
