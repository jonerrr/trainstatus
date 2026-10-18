<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop } from '#lib/client/index.js';
	import Option from '#lib/Transfers/Option.svelte';
	import {
		boardingOptions,
		compareOptions,
		departureOptions,
		type IncomingTrip
	} from '#lib/transit/nearby.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	let {
		stop,
		incoming,
		time_format
	}: { stop: CatalogStop; incoming: IncomingTrip; time_format: 'time' | 'countdown' } = $props();
	const transit = getTransitData();
	const current_time = getCurrentTime();
	let expanded = $state(false);
	let requested_all = $state(false);
	const catalogs_resource = transit.catalogs(() => page.data.selected_sources);
	const catalogs = $derived(
		catalogs_resource.current.flatMap((read) => (read.catalog ? [read.catalog] : []))
	);
	const options = $derived(boardingOptions(stop, catalogs, incoming));
	const requested = $derived(requested_all ? options : options.slice(0, 3));
	const routes_resource = transit.routes(() => [
		...new Map(
			requested.map((o) => [
				JSON.stringify([o.route.data.source, o.route.id]),
				{ source: o.route.data.source, route_id: o.route.id, at: current_time.value }
			])
		).values()
	]);
	const reads = $derived(routes_resource.current);
	const departures = $derived(
		departureOptions(requested, reads, incoming, catalogs, current_time.ms)
	);
	const ordered = $derived(departures.filter((o) => o.call).sort(compareOptions));
	const pending = $derived(reads.some((r) => !r.route && r.loading && !r.error));
	const visible = $derived(expanded ? ordered : ordered.slice(0, 3));
	const has_details = $derived(
		expanded ||
			(!requested_all && options.length > requested.length) ||
			ordered.length > 3 ||
			ordered.some((o) => o.next)
	);
	const partial_error = $derived(reads.some((r) => r.error));
	function toggle() {
		expanded = !expanded;
		if (expanded) requested_all = true;
	}
</script>

<section class="bg-neutral-900 px-2" aria-label="Transfers after arrival">
	<div class="flex items-baseline justify-between gap-2 border-b border-neutral-800 px-1 py-2">
		<h3 class="text-sm font-medium">Transfers after arrival</h3>
		<span class="text-xs text-neutral-400"
			>{new Date(incoming.arrival).toLocaleTimeString([], {
				hour: 'numeric',
				minute: '2-digit'
			})}</span
		>
	</div>
	{#each visible as departure (departure.key)}
		<Option
			option={departure}
			{departure}
			{time_format}
			arrival={incoming.arrival}
			show_source={options.some(
				(o) =>
					o.route.short_name === departure.route.short_name &&
					o.route.data.source !== departure.route.data.source
			)}
		/>
	{:else}<p class="px-1 py-3 text-xs text-neutral-400" role="status">
			{pending ? 'Loading predictions…' : 'No transfer predictions to show'}
		</p>{/each}
	{#if partial_error}<p class="px-1 py-1 text-xs text-neutral-400" role="status">
			Some predictions unavailable
		</p>{/if}
	{#if expanded && ordered.some((o) => o.next)}
		<div class="mt-3 ml-2 border-l border-neutral-700 pl-2 text-xs text-neutral-400">
			<h4 class="px-1 py-1 font-medium">Later departures</h4>
			{#each ordered as departure (departure.key)}
				{#if departure.next}<Option
						option={departure.next}
						departure={departure.next}
						{time_format}
						arrival={incoming.arrival}
					/>{/if}
			{/each}
		</div>
	{/if}
	{#if has_details}
		<button
			type="button"
			class="mt-2 min-h-11 w-full border-t border-neutral-700 text-center text-xs font-medium text-neutral-300 hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-blue-400"
			aria-expanded={expanded}
			onclick={toggle}
		>
			{expanded ? 'Show less' : 'Show more'}
		</button>
	{/if}
</section>
