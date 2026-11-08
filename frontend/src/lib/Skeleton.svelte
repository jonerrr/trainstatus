<script lang="ts">
	interface Props {
		lines?: number;
		class?: string;
		// matches the line height of the text being replaced
		height?: string;
	}

	let { lines = 3, class: className = '', height = 'h-4' }: Props = $props();
</script>

<div class="flex flex-col gap-2 {className}">
	{#each Array.from({ length: lines }, (_, i) => i) as i (i)}
		<div
			class="{height} animate-pulse rounded-none bg-surface-2"
			style:width="{75 + ((i * 17) % 25)}%"
		></div>
	{/each}
</div>
