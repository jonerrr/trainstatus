<script lang="ts">
	import { tick } from 'svelte';

	import type { Attachment } from 'svelte/attachments';

	import { page } from '$app/state';

	import { source_info } from '$lib/resources/index.svelte';

	import { Layers, RotateCcw, SlidersHorizontal, X } from '@lucide/svelte';

	import MapBackdrop from './MapBackdrop.svelte';
	import SourceFilterGroup from './SourceFilterGroup.svelte';
	import { dismissOnEscape } from './dialog';
	import type { FilterValue, MapFilters } from './filters.svelte';
	import { layer_data } from './filters.svelte';

	// TODO: simplify the code and improve the ui
	// there should be less padding, i dont like how its called filters but the title is "map settings"
	// it also doesn't look fully aligned with the dropdown. maybe it should be connected to the sidebar (or bottom nav on mobile)
	// and (on desktop at least), the dropdown shouldnt close until they click close

	interface Props {
		filters: MapFilters;
	}

	type LayerKey = keyof MapFilters['layers'];
	const layerEntries = Object.entries(layer_data) as [LayerKey, (typeof layer_data)[LayerKey]][];

	let { filters = $bindable() }: Props = $props();

	let filtersOpen = $state(false);
	let trigger = $state<HTMLButtonElement>();
	let panel = $state<HTMLDivElement>();

	const captureTrigger: Attachment<HTMLButtonElement> = (node) => {
		trigger = node;
		return () => {
			if (trigger === node) trigger = undefined;
		};
	};

	const capturePanel: Attachment<HTMLDivElement> = (node) => {
		panel = node;
		return () => {
			if (panel === node) panel = undefined;
		};
	};

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
		if (!filtersOpen) return;
		if (dismissOnEscape(event, () => closeFilters())) return;
		if (event.key !== 'Tab') return;

		const focusable = Array.from(
			panel?.querySelectorAll<HTMLElement>(
				'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
			) ?? []
		).filter((element) => element.offsetParent !== null);
		if (focusable.length === 0) return;
		const first = focusable[0];
		const last = focusable.at(-1)!;
		if (event.shiftKey && document.activeElement === first) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && document.activeElement === last) {
			event.preventDefault();
			first.focus();
		}
	}

	function handleOutsideClick(event: PointerEvent) {
		const target = event.target as Node;
		if (filtersOpen && !panel?.contains(target) && !trigger?.contains(target)) closeFilters();
	}
</script>

<svelte:window onkeydown={handleKeydown} onpointerdown={handleOutsideClick} />

<div class="pointer-events-none absolute top-1.5 left-1.5 z-70 max-w-[calc(100%-4.25rem)]">
	<div
		class="pointer-events-auto flex max-w-full items-center gap-1 rounded-lg border border-neutral-700/70 bg-neutral-950/88 p-1 text-white shadow-lg backdrop-blur-md"
	>
		<button
			{@attach captureTrigger}
			type="button"
			class="relative flex h-9 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs font-semibold hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-400"
			aria-label="Map settings"
			aria-expanded={filtersOpen}
			aria-controls="map-filter-panel"
			onclick={() => (filtersOpen ? closeFilters() : void openFilters())}
		>
			<SlidersHorizontal size={18} aria-hidden="true" />
			<span class="hidden sm:inline">Filters</span>
			{#if filters.activeFilterCount > 0}
				<span
					class="flex min-w-4 items-center justify-center rounded-full bg-blue-500 px-1 text-[10px] font-bold leading-4 text-white"
					aria-label={`${filters.activeFilterCount} active filters`}
				>
					{filters.activeFilterCount}
				</span>
			{/if}
		</button>

		<div class="h-5 w-px shrink-0 bg-neutral-700"></div>
		<div class="flex min-w-0 gap-0.5 overflow-x-auto" aria-label="Data sources">
			{#each page.data.selected_sources as source (source)}
				{@const info = source_info[source]}
				{@const enabled = filters.isSourceEnabled(source)}
				<button
					type="button"
					class={`flex h-9 shrink-0 items-center gap-1 rounded-md border px-1.5 text-xs font-medium focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-400 ${enabled ? 'border-blue-400 bg-blue-500/20' : 'border-neutral-700 text-neutral-400'}`}
					aria-pressed={enabled}
					aria-label={`${enabled ? 'Hide' : 'Show'} ${info.name}`}
					onclick={() => filters.toggleSource(source)}
				>
					<img src={info.icon} alt="" class="size-5 rounded object-contain" />
					<span class="hidden lg:inline">{info.name.replace('MTA ', '')}</span>
				</button>
			{/each}
		</div>
	</div>
</div>

{#if filtersOpen}
	<MapBackdrop
		class="filter-backdrop"
		label="Close map settings"
		ondismiss={() => closeFilters()}
	/>
	<div
		{@attach capturePanel}
		id="map-filter-panel"
		class="filter-panel"
		role="dialog"
		aria-modal="true"
		aria-label="Map settings"
	>
		<header
			class="flex min-h-11 shrink-0 items-center justify-between border-b border-neutral-800 px-3"
		>
			<div class="flex items-center gap-1.5">
				<Layers size={18} aria-hidden="true" />
				<h2 class="text-base font-semibold">Map settings</h2>
			</div>
			<button
				type="button"
				class="flex size-9 items-center justify-center rounded-md hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-blue-400"
				aria-label="Close map settings"
				onclick={() => closeFilters()}
			>
				<X size={20} aria-hidden="true" />
			</button>
		</header>

		<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
			<section class="flex flex-col gap-1" aria-labelledby="map-layers-heading">
				<h3
					id="map-layers-heading"
					class="mb-1 text-xs font-semibold tracking-wide text-neutral-400 uppercase"
				>
					Layers
				</h3>
				{#each layerEntries as [layer, { name }] (layer)}
					<label
						class="flex min-h-10 items-center justify-between gap-2 rounded-md px-1.5 hover:bg-neutral-800/70"
					>
						<span class="text-sm">{name}</span>
						<input
							bind:checked={filters.layers[layer]}
							type="checkbox"
							class="size-5 accent-blue-500"
						/>
					</label>
				{/each}
			</section>

			{#each layerEntries as [layer] (layer)}
				{#if filters.layers[layer] && filters.sources.length > 0}
					<section class="flex flex-col gap-1.5 border-t border-neutral-800 pt-3">
						<h3 class="text-xs font-semibold tracking-wide text-neutral-400 uppercase">
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
			class="shrink-0 border-t border-neutral-800 p-2 pb-[calc(0.5rem+env(safe-area-inset-bottom))]"
		>
			<button
				type="button"
				class="flex min-h-10 w-full items-center justify-center gap-1.5 rounded-md border border-neutral-700 text-sm font-semibold hover:bg-neutral-800 disabled:cursor-not-allowed disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-blue-400"
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
	:global(.filter-backdrop) {
		position: fixed;
		inset: 0;
		z-index: 71;
		background: rgb(0 0 0 / 0.35);
	}

	.filter-panel {
		position: fixed;
		z-index: 72;
		right: 0;
		bottom: 0;
		left: 0;
		display: flex;
		max-height: min(75vh, 42rem);
		flex-direction: column;
		border: 1px solid rgb(64 64 64);
		border-bottom: 0;
		border-radius: 0.75rem 0.75rem 0 0;
		background: rgb(10 10 10 / 0.97);
		color: white;
		box-shadow: 0 -20px 50px rgb(0 0 0 / 0.42);
		backdrop-filter: blur(16px);
	}

	@media (min-width: 768px) {
		:global(.filter-backdrop) {
			left: 5rem;
			background: transparent;
		}

		.filter-panel {
			top: 3.25rem;
			right: auto;
			bottom: auto;
			left: 5.375rem;
			width: 20rem;
			max-height: calc(100% - 3.625rem);
			border-bottom: 1px solid rgb(64 64 64);
			border-radius: 0.65rem;
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
