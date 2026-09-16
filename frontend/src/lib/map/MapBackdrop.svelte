<script lang="ts">
	import type { ClassValue } from 'svelte/elements';

	let {
		ondismiss,
		label,
		class: className
	}: {
		ondismiss: () => void;
		label: string;
		class?: ClassValue;
	} = $props();
</script>

<button
	type="button"
	tabindex="-1"
	class={['map-backdrop', className]}
	aria-label={label}
	onclick={ondismiss}
></button>

<style>
	.map-backdrop {
		border: 0;
		padding: 0;
	}
</style>
