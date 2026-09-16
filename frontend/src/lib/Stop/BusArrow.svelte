<script lang="ts">
	import type { CompassDirection } from '$lib/client';
	import { COMPASS_DIRECTIONS } from '$lib/compassDirections';

	import { ArrowUp } from '@lucide/svelte';

	const { direction, size = '1.5rem' }: { direction: CompassDirection; size?: string } = $props();
	const rotation = $derived(COMPASS_DIRECTIONS[direction].rotation);
</script>

<div>
	{#if rotation !== null}
		<ArrowUp {size} style={`transform: rotate(${rotation}deg)`} />
	{/if}
</div>
