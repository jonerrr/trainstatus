<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { trip_destination_title, trip_headsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { trip_status_summary, trip_vehicle_summary } from './status.js';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		source: Source;
		route_id: string;
		trip_id: string;
	}
	let { source, route_id, trip_id }: Props = $props();

	const at = $derived(current_time.value);
	const catalog_resource = transit.catalog(() => source);
	const route_resource = transit.route(() => ({ source, route_id, at }));
	const catalog_read = $derived(catalog_resource.current);
	const route_read = $derived(route_resource.current);
	const catalog = $derived(catalog_read.state === 'ready' ? catalog_read.value : undefined);
	const route_data = $derived(route_read.state === 'ready' ? route_read.value : undefined);
	const route = $derived(catalog?.routes_by_id.get(route_id));

	// A pin outlives the realtime run it refers to.
	const read = $derived(
		route_data?.trips.has(trip_id)
			? {
					trip: route_data.trips.get(trip_id)!,
					calls: route_data.calls_by_trip.get(trip_id) ?? []
				}
			: undefined
	);

	const destination = $derived(
		trip_destination_title(
			read ? trip_headsign(read.trip, route, read.calls, catalog?.stops_by_id) : undefined,
			route
		)
	);

	const summary = $derived(
		read
			? trip_status_summary(read.trip, read.calls, catalog?.stops_by_id, current_time.ms)
			: undefined
	);

	const vehicle = $derived(read ? trip_vehicle_summary(read.trip) : undefined);
	const is_bus = $derived(source === 'mta_bus' || source === 'njt_bus');
</script>

<div class="flex min-h-17 w-full min-w-0 items-start">
	{#if (!catalog || !route_data) && (catalog_read.state === 'loading' || route_read.state === 'loading')}
		<Skeleton lines={2} class="w-full" height="h-6" />
	{:else if !catalog || !route_data}
		<span class="text-muted">—</span>
	{:else}
		<div class="flex w-full min-w-0 items-start gap-2.5 pr-8 text-left">
			{#if route}
				<Icon width={28} height={28} {route} link={false} class="mt-0.5 shrink-0" />
			{/if}

			<div class="flex min-w-0 flex-1 flex-col gap-0.5">
				<div class="flex items-baseline justify-between gap-2">
					<span class="truncate text-base font-semibold text-primary">
						{destination}
					</span>
					{#if vehicle?.vehicle_label}
						<span class="shrink-0 text-xs text-muted tabular-nums">
							{vehicle.vehicle_label}
						</span>
					{/if}
				</div>

				{#if summary}
					<div class="flex items-center justify-between gap-2 text-sm">
						<span class="truncate text-secondary">
							{summary.description}
						</span>
						{#if summary.eta_label}
							<span
								class="shrink-0 rounded-none bg-surface-2 px-1.5 py-0.5 text-xs font-semibold text-primary tabular-nums"
							>
								{summary.eta_label}
							</span>
						{/if}
					</div>

					{#if vehicle?.deviation_text || (is_bus && read?.trip.status)}
						<div class="flex items-center gap-2 text-xs text-muted">
							{#if vehicle?.deviation_text}
								<span
									class={vehicle.deviation_tone === 'warning'
										? 'text-status-warning'
										: 'text-secondary'}
								>
									{vehicle.deviation_text}
								</span>
							{/if}

							{#if is_bus && read?.trip.status}
								<div class="ml-auto">
									<VehicleCapacity status={read.trip.status} />
								</div>
							{/if}
						</div>
					{/if}
				{:else}
					<div class="text-xs text-muted">Trip not running</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
