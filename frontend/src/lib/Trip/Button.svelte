<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { trip_headsign } from '#lib/util.svelte.js';

	import { ArrowBigRight } from '@lucide/svelte';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		source: Source;
		route_id: string;
		trip_id: string;
	}
	let { source, route_id, trip_id }: Props = $props();

	const at = $derived(current_time.value);
	// Rows render as their route arrives; a suspending read per row would batch them.
	const catalog_resource = transit.catalog(() => source);
	const route_resource = transit.route(() => ({ source, route_id, at }));
	const catalog_state = $derived(catalog_resource.current);
	const route_state = $derived(route_resource.current);
	const catalog = $derived(catalog_state.value);
	const route_data = $derived(route_state.value);
	const error = $derived(catalog_state.error ?? route_state.error);
	// A pin outlives the realtime run it refers to.
	const read = $derived(
		route_data?.trips.has(trip_id)
			? {
					trip: route_data.trips.get(trip_id)!,
					calls: route_data.calls_by_trip.get(trip_id) ?? []
				}
			: undefined
	);
	const route = $derived(catalog?.routes_by_id.get(route_id));
</script>

{#if error}
	<span class="text-muted">—</span>
{:else if !catalog || !route_data}
	<Skeleton lines={2} class="w-full" />
{:else}
	<div class="flex flex-col items-center gap-1 text-left">
		<div class="flex max-w-[95%] items-center gap-1 self-start">
			{#if route}
				<Icon width={36} height={36} {route} link={false} />
			{/if}

			<ArrowBigRight />

			{#if read}
				{trip_headsign(read.trip, route, read.calls, catalog.stops_by_id)}
			{:else}
				Trip not running
			{/if}
		</div>

		{#if read}
			{const upcoming = $derived(read.calls.filter((call) => call.arrival > current_time.ms))}
			{const status = $derived(read.trip.status)}
			<div class="flex gap-1 self-start">
				{#if status && 'status' in status.data && status.data.status}
					<span>{status.data.status}</span>
				{/if}
				{catalog.stops_by_id.get(status?.stop_id || upcoming[0]?.stop_id)?.name ?? 'Unknown'}
			</div>
		{/if}
	</div>
{/if}
