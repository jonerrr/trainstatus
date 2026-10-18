<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop } from '#lib/client/index.js';
	import Option from '#lib/Transfers/Option.svelte';
	import { boardingOptions } from '#lib/transit/nearby.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';

	let { stop }: { stop: CatalogStop } = $props();
	let expanded = $state(false);
	const options_id = $props.id();
	const transit = getTransitData();
	const catalogs_resource = transit.catalogs(() => page.data.selected_sources);
	const catalogs = $derived(
		catalogs_resource.current.flatMap((read) => (read.catalog ? [read.catalog] : []))
	);
	const options = $derived(boardingOptions(stop, catalogs));
	const visible = $derived(expanded ? options : options.slice(0, 3));
</script>

{#if options.length}
	<section class="shrink-0 border-b border-neutral-700 px-3 pb-2" aria-label="Nearby routes">
		<div class="flex min-h-11 items-center justify-between gap-2">
			<h3 class="text-xs font-medium text-neutral-400">Nearby routes</h3>
			{#if options.length > 3}
				<button
					type="button"
					class="min-h-11 px-2 text-xs font-medium text-neutral-300 hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-blue-400"
					aria-expanded={expanded}
					aria-controls={options_id}
					onclick={() => (expanded = !expanded)}
				>
					{expanded ? 'Show less' : `Show all ${options.length} options`}
				</button>
			{/if}
		</div>
		<div
			id={options_id}
			class="max-h-[min(12rem,25dvh)] overflow-y-auto overscroll-contain"
			role="region"
			aria-label="Nearby route options"
		>
			{#each visible as option (option.key)}<Option
					{option}
					show_stop={option.stops[0].stop.name !== stop.name}
					show_source={options.some(
						(o) =>
							o.route.short_name === option.route.short_name &&
							o.route.data.source !== option.route.data.source
					)}
				/>{/each}
		</div>
	</section>
{/if}
