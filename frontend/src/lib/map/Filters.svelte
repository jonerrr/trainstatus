<script lang="ts">
	import { tick } from 'svelte';

	import { page } from '$app/state';

	import { source_icons } from '#lib/source_icons.js';
	import { source_config } from '#lib/sources.js';

	import { Layers, RotateCcw, SlidersHorizontal, X } from '@lucide/svelte';

	import { layer_data, type FilterValue, type MapFilters } from './filters.svelte';
	import SourceFilterGroup from './SourceFilterGroup.svelte';

	interface Props {
		filters: MapFilters;
	}

	type LayerKey = keyof MapFilters['layers'];
	const layerEntries = Object.entries(layer_data) as [LayerKey, (typeof layer_data)[LayerKey]][];

	let { filters = $bindable() }: Props = $props();

	let filtersOpen = $state(false);
	let trigger = $state<HTMLButtonElement>();
	let panel = $state<HTMLDivElement>();

	async function openFilters() {
		filtersOpen = true;
		await tick();
		panel?.querySelector<HTMLElement>('button, input, select, textarea')?.focus();
	}

	function closeFilters({ restoreFocus = true } = {}) {
		if (!filtersOpen) return;
		filtersOpen = false;
		if (restoreFocus) void tick().then(() => trigger?.focus());
	}

	function handleKeydown(event: KeyboardEvent) {
		if (
			event.key === 'Escape' &&
			filtersOpen &&
			!event.defaultPrevented &&
			!page.state.modal &&
			(panel?.contains(document.activeElement) || trigger === document.activeElement)
		) {
			event.preventDefault();
			closeFilters();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="pointer-events-none absolute top-1.5 left-1.5 z-30 max-w-[calc(100%-4.25rem)]">
	<div
		class="pointer-events-auto flex max-w-full items-center gap-1 rounded-none border border-border-default/70 bg-canvas/88 p-1 text-white shadow-lg backdrop-blur-md"
	>
		<button
			bind:this={trigger}
			type="button"
			class="relative flex h-9 shrink-0 items-center gap-1.5 rounded-none px-2 text-xs font-semibold hover:bg-hover focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
			aria-label="Filters"
			aria-expanded={filtersOpen}
			aria-controls="map-filter-panel"
			onclick={() => (filtersOpen ? closeFilters() : void openFilters())}
		>
			<SlidersHorizontal size={18} aria-hidden="true" />
			<span class="hidden sm:inline">Filters</span>
			{#if filters.activeFilterCount > 0}
				<span
					class="flex min-w-4 items-center justify-center rounded-none bg-accent px-1 text-[10px] leading-4 font-bold text-accent-fg"
					aria-label={`${filters.activeFilterCount} active filters`}
				>
					{filters.activeFilterCount}
				</span>
			{/if}
		</button>

		<div class="h-5 w-px shrink-0 bg-border-default"></div>
		<div class="flex min-w-0 gap-0.5 overflow-x-auto" aria-label="Data sources">
			{#each page.data.selected_sources as source (source)}
				{const info = $derived(source_config[source])}
				{const enabled = $derived(filters.isSourceEnabled(source))}
				<button
					type="button"
					class={`flex h-9 shrink-0 items-center gap-1 rounded-none border px-1.5 text-xs font-medium focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent ${enabled ? 'border-accent-border bg-accent-bg' : 'border-border-default text-muted'}`}
					aria-pressed={enabled}
					aria-label={`${enabled ? 'Hide' : 'Show'} ${info.name}`}
					onclick={() => filters.toggleSource(source)}
				>
					<enhanced:img
						src={source_icons[source]}
						sizes="20px"
						alt=""
						class="size-5 rounded-none object-contain"
					/>
					<span class="hidden lg:inline">{info.name.replace('MTA ', '')}</span>
				</button>
			{/each}
		</div>
	</div>
</div>

{#if filtersOpen}
	<div
		bind:this={panel}
		id="map-filter-panel"
		class="filter-panel"
		role="dialog"
		aria-label="Filters"
	>
		<header
			class="flex min-h-11 shrink-0 items-center justify-between border-b border-border-subtle px-3"
		>
			<div class="flex items-center gap-1.5">
				<Layers size={18} aria-hidden="true" />
				<h2 class="text-base font-semibold">Filters</h2>
			</div>
			<button
				type="button"
				class="flex size-9 items-center justify-center rounded-none hover:bg-hover focus-visible:outline-2 focus-visible:outline-accent"
				aria-label="Close filters"
				onclick={() => closeFilters()}
			>
				<X size={20} aria-hidden="true" />
			</button>
		</header>

		<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
			<section class="flex flex-col gap-1" aria-labelledby="map-layers-heading">
				<h3
					id="map-layers-heading"
					class="mb-1 text-xs font-semibold tracking-wide text-muted uppercase"
				>
					Layers
				</h3>
				{#each layerEntries as [layer, { name }] (layer)}
					<label
						class="flex min-h-10 items-center justify-between gap-2 rounded-none px-1.5 hover:bg-hover/70"
					>
						<span class="text-sm">{name}</span>
						<input
							bind:checked={filters.layers[layer]}
							type="checkbox"
							class="size-5 accent-accent"
						/>
					</label>
				{/each}
			</section>

			{#each layerEntries as [layer] (layer)}
				{#if filters.layers[layer] && filters.sources.length > 0}
					<section class="flex flex-col gap-1.5 border-t border-border-subtle pt-3">
						<h3 class="text-xs font-semibold tracking-wide text-muted uppercase">
							{layer} filters
						</h3>
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
							{:else}
								<SourceFilterGroup
									{layer}
									{source}
									bind:filters={filters.trip_filters[source] as Record<string, FilterValue>}
								/>
							{/if}
						{/each}
					</section>
				{/if}
			{/each}
		</div>

		<footer
			class="shrink-0 border-t border-border-subtle p-2 pb-[calc(0.5rem+env(safe-area-inset-bottom))]"
		>
			<button
				type="button"
				class="flex min-h-10 w-full items-center justify-center gap-1.5 rounded-none border border-border-default text-sm font-semibold hover:bg-hover focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-40"
				disabled={filters.activeFilterCount === 0}
				onclick={() => filters.reset()}
			>
				<RotateCcw size={17} aria-hidden="true" />
				Reset filters
			</button>
		</footer>
	</div>
{/if}

<style>
	.filter-panel {
		position: absolute;
		z-index: 30;
		right: 0.5rem;
		bottom: 0.5rem;
		left: 0.5rem;
		display: flex;
		max-height: min(48dvh, 30rem);
		flex-direction: column;
		border: 1px solid var(--border-default);
		border-bottom: 1px solid var(--border-default);
		border-radius: 0;
		background: var(--bg-canvas);
		color: white;
		box-shadow: 0 -20px 50px rgb(0 0 0 / 0.42);
		backdrop-filter: blur(16px);
	}

	@media (min-width: 768px) {
		.filter-panel {
			top: 3.25rem;
			right: auto;
			bottom: auto;
			left: 0.375rem;
			width: 20rem;
			max-height: calc(100% - 3.625rem);
			border-bottom: 1px solid var(--border-default);
			border-radius: 0;
			box-shadow: 0 20px 50px rgb(0 0 0 / 0.38);
		}
	}

	@media (prefers-reduced-motion: no-preference) {
		.filter-panel {
			animation: filter-sheet-in 160ms ease-out;
		}
	}

	@keyframes filter-sheet-in {
		from {
			opacity: 0;
			transform: translateY(0.75rem);
		}
	}
</style>
