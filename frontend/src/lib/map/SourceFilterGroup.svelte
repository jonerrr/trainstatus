<script lang="ts">
	import { slide } from 'svelte/transition';

	import type { Source } from '$lib/client';
	import FilterField from '$lib/map/FilterField.svelte';
	import {
		type FilterFieldDef,
		type FilterValue,
		getFilterDefsForLayer
	} from '$lib/map/filters.svelte';

	interface Props {
		layer: 'route' | 'stop' | 'trip';
		source: Source;
		filters: Record<string, FilterValue>;
	}

	let { layer, source, filters = $bindable() }: Props = $props();

	const filterDefs = $derived(getFilterDefsForLayer(layer));
	const sourceFilterDefs = $derived(filterDefs[source]);
	const entries = $derived(Object.entries(sourceFilterDefs) as [string, FilterFieldDef][]);

	const sourceLabel = $derived(source.replace('_', ' '));
	let groupOpen = $state(true);
</script>

{#if entries.length > 0}
	<div
		class="flex flex-col gap-2 p-2 rounded border border-neutral-300 dark:border-neutral-600 bg-neutral-50 dark:bg-neutral-900"
	>
		<div class="flex items-center justify-between gap-2">
			<div class="text-sm font-semibold capitalize">{sourceLabel}</div>
			<button
				type="button"
				class="text-xs underline text-blue-500 hover:text-blue-700"
				onclick={() => {
					groupOpen = !groupOpen;
				}}
			>
				{groupOpen ? 'Hide' : 'Show'}
			</button>
		</div>

		{#if groupOpen}
			<div class="flex flex-col gap-2" transition:slide>
				{#each entries as [property, fieldDef] (property)}
					<FilterField
						label={fieldDef.label}
						{fieldDef}
						value={filters[property] as FilterValue}
						onChange={(newValue) => {
							if (newValue === undefined) {
								delete filters[property];
							} else {
								filters[property] = newValue;
							}
						}}
					/>
				{/each}
			</div>
		{/if}
	</div>
{/if}
