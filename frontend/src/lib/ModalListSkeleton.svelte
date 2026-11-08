<script lang="ts">
	import ModalList from '#lib/ModalList.svelte';

	interface Props {
		rows?: number;
		// Trip stop rows start with the 44px timeline column.
		timeline?: boolean;
	}

	let { rows = 5, timeline = false }: Props = $props();
</script>

<ModalList>
	{#each Array.from({ length: rows }, (_, i) => i) as i (i)}
		<div class="flex min-h-11 items-center gap-2 pr-2 {timeline ? '' : 'pl-2'}" aria-hidden="true">
			{#if timeline}
				<div class="flex w-11 shrink-0 justify-center self-stretch">
					<div class="w-1 bg-surface-2"></div>
				</div>
			{/if}
			<div
				class="h-5 animate-pulse rounded-none bg-surface-2"
				style:width="{45 + ((i * 17) % 30)}%"
			></div>
			<div class="ml-auto h-5 w-10 shrink-0 animate-pulse rounded-none bg-surface-2"></div>
		</div>
	{/each}
</ModalList>
