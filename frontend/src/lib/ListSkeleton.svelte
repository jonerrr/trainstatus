<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import { skeleton_row_height } from '#lib/util.svelte.js';

	interface Props {
		type: 'stop' | 'route' | 'trip';
		source?: Source;
		count: number;
	}

	let { type, source, count }: Props = $props();

	// Same constants as the virtual list's height calculators, so rows don't shift on load.
	const height = $derived(skeleton_row_height(type, source));
</script>

{#snippet bar(class_name: string)}
	<div class="animate-pulse rounded-none bg-surface-2 {class_name}"></div>
{/snippet}

<div aria-hidden="true">
	{#each Array.from({ length: count }, (_, i) => i) as i (i)}
		<div
			style:height="{height}px"
			class="box-border flex w-full flex-col justify-center rounded-none border border-border-subtle/50 bg-canvas p-2"
		>
			{#if type === 'route'}
				<div class="flex items-center gap-1">
					{@render bar('size-9')}
					{@render bar('h-6 w-24')}
				</div>
			{:else if type === 'trip'}
				<div class="flex flex-col gap-1">
					<div class="flex items-center gap-1">
						{@render bar('size-9')}
						{@render bar('h-6 w-48')}
					</div>
					{@render bar('h-6 w-32')}
				</div>
			{:else if source === 'mta_subway'}
				<div class="flex flex-col gap-1">
					<div class="flex items-center gap-1">
						{@render bar('size-6')}
						{@render bar('h-7 w-40')}
					</div>
					<div class="grid grid-cols-2 gap-8">
						{#each [0, 1] as column (column)}
							<div class="flex flex-col gap-1">
								{@render bar('h-6 w-3/4')}
								{@render bar('h-6 w-20')}
							</div>
						{/each}
					</div>
				</div>
			{:else}
				<div class="flex flex-col gap-1">
					{@render bar('h-7 w-48')}
					<div class="flex items-center gap-2">
						{@render bar('size-8')}
						<div class="flex flex-col gap-1">
							{@render bar('h-6 w-40')}
							{@render bar('h-6 w-20')}
						</div>
					</div>
				</div>
			{/if}
		</div>
	{/each}
</div>
