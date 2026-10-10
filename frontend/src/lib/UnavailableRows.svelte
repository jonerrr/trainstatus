<script lang="ts">
	interface Props {
		arrivals?: boolean;
	}

	let { arrivals = false }: Props = $props();
</script>

{#if arrivals}
	<p class="p-2 text-center text-neutral-400">Arrivals unavailable</p>
{:else}
	<span class="text-xs text-amber-200">Unavailable</span>
{/if}
