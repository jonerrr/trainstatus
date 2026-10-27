<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import FilterField from '#lib/map/FilterField.svelte';
	import {
		getFilterDefsForLayer,
		type FilterFieldDef,
		type FilterValue
	} from '#lib/map/filters.svelte.js';

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
		class="flex flex-col gap-1.5 rounded-none border border-border-subtle bg-surface-1 p-1.5 text-primary"
	>
		<div class="flex min-h-10 items-center justify-between gap-2">
			<div class="text-sm font-semibold capitalize">{sourceLabel}</div>
			<button
				type="button"
				class="min-h-10 rounded-none px-1.5 text-xs text-accent underline hover:text-accent"
				onclick={() => {
					groupOpen = !groupOpen;
				}}
			>
				{groupOpen ? 'Hide' : 'Show'}
			</button>
		</div>

		{#if groupOpen}
			<div class="flex flex-col gap-1.5">
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
