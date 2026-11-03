<script lang="ts">
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
	class={`flex max-h-[60dvh] flex-col divide-y divide-border-subtle overflow-auto border-y border-border-subtle bg-canvas text-base ${class_name ?? ''}`}
>
	{@render children()}
</div>
