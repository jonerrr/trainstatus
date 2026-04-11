<script lang="ts">
	interface Props {
		lines?: number;
		class?: string;
	}

	let { lines = 3, class: className = '' }: Props = $props();
</script>

<div class="flex flex-col gap-2 {className}">
	{#each { length: lines } as _, i}
		<div
			class="h-4 animate-pulse rounded bg-neutral-800"
			style:width="{75 + ((i * 17) % 25)}%"
		></div>
	{/each}
</div>
