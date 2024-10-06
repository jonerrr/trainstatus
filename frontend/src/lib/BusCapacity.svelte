<script lang="ts">
	import { Users } from 'lucide-svelte';

	const { passengers, capacity }: { passengers: number; capacity: number } = $props();

	const p_full = $derived(Math.floor((passengers / capacity) * 100));
</script>

<div
	class={`flex items-center gap-1 text-neutral-200 ${p_full > 30 ? 'text-yellow-400' : ''} ${p_full > 80 ? 'text-red-400' : ''}`}
>
	<Users size="16" />
	{passengers}
</div>
