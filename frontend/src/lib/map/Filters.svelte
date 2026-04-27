<script lang="ts">
	import { slide } from 'svelte/transition';

	import SourceFilterGroup from './SourceFilterGroup.svelte';
	import SourceSelector from './SourceSelector.svelte';
	import type { FilterValue, MapFilters } from './filters.svelte';
	import { layer_data } from './filters.svelte';

	interface Props {
		filters: MapFilters;
	}

	type LayerKey = keyof MapFilters['layers'];
	const layerEntries = Object.entries(layer_data) as [LayerKey, (typeof layer_data)[LayerKey]][];

	let { filters = $bindable() }: Props = $props();

	let filters_open = $state(false);
</script>

<!-- TODO: connect to map route -->
<div class="absolute z-50 top-0 left-0">
	<div
		class="flex flex-col gap-2 p-2 m-2 rounded-lg bg-white/50 dark:bg-black/50 backdrop-blur-md w-80 max-h-[calc(100vh-1rem)]"
	>
		<div class="flex justify-between gap-2 items-center">
			<div class="text-lg font-semibold">Filters</div>
			<button
				onclick={() => {
					filters_open = !filters_open;
				}}
				class="underline text-sm text-blue-500 hover:text-blue-700"
				>{filters_open ? 'Hide' : 'Show'}</button
			>
		</div>

		{#if filters_open}
			<div class="flex flex-col gap-3 overflow-y-auto pr-1 max-h-[calc(100vh-7rem)]" transition:slide>
				<!-- Layer toggles -->
				<div class="flex flex-col gap-2 pb-2 border-b border-neutral-300 dark:border-neutral-600">
					<div class="text-sm font-semibold text-gray-700 dark:text-gray-300">Layers</div>
					{#each layerEntries as [layer, { name }] (layer)}
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span class="text-sm">{name}</span>
							<input bind:checked={filters.layers[layer]} type="checkbox" />
						</label>
					{/each}
				</div>

				<!-- Source selection -->
				<div>
					<SourceSelector bind:sources={filters.sources} />
				</div>

				<!-- Layer-specific filters -->
				{#each layerEntries as [layer] (layer)}
					{#if filters.layers[layer] && filters.sources.length > 0}
						<div
							class="flex flex-col gap-2 pb-2 border-b border-neutral-300 dark:border-neutral-600"
						>
							<div class="text-sm font-semibold text-gray-700 dark:text-gray-300 capitalize">
								{layer} Filters
							</div>
							{#each filters.sources as source (source)}
								{#if layer === 'stop'}
									<SourceFilterGroup
										{layer}
										{source}
										bind:filters={filters.stop_filters[source] as Record<string, FilterValue>}
									/>
								{:else if layer === 'route'}
									<SourceFilterGroup
										{layer}
										{source}
										bind:filters={filters.route_filters[source] as Record<string, FilterValue>}
									/>
								{:else if layer === 'trip'}
									<SourceFilterGroup
										{layer}
										{source}
										bind:filters={filters.trip_filters[source] as Record<string, FilterValue>}
									/>
								{/if}
							{/each}
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
</div>
