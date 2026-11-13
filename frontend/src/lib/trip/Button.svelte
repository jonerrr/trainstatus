<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import Icon from '#lib/route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';
	import { tripDestinationTitle, tripHeadsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { tripStatusSummary, tripVehicleSummary } from './status.js';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		source: Source;
		routeId: string;
		tripId: string;
	}
	let { source, routeId, tripId }: Props = $props();

	const at = $derived(currentTime.value);
	const catalogResource = transit.catalog(() => source);
	const routeResource = transit.route(() => ({ source, routeId, at }));
	const catalogRead = $derived(catalogResource.current);
	const routeRead = $derived(routeResource.current);
	const catalog = $derived(catalogRead.state === 'ready' ? catalogRead.value : undefined);
	const routeData = $derived(routeRead.state === 'ready' ? routeRead.value : undefined);
	const route = $derived(catalog?.routesById.get(routeId));

	// A pin outlives the realtime run it refers to.
	const read = $derived(
		routeData?.trips.has(tripId)
			? {
					trip: routeData.trips.get(tripId)!,
					calls: routeData.callsByTrip.get(tripId) ?? []
				}
			: undefined
	);

	const destination = $derived(
		tripDestinationTitle(
			read ? tripHeadsign(read.trip, route, read.calls, catalog?.stopsById) : undefined,
			route
		)
	);

	const summary = $derived(
		read ? tripStatusSummary(read.trip, read.calls, catalog?.stopsById, currentTime.ms) : undefined
	);

	const vehicle = $derived(read ? tripVehicleSummary(read.trip) : undefined);
	const isBus = $derived(source === 'mta_bus' || source === 'njt_bus');
</script>

<div class="flex min-h-17 w-full min-w-0 items-start">
	{#if (!catalog || !routeData) && (catalogRead.state === 'loading' || routeRead.state === 'loading')}
		<Skeleton lines={2} class="w-full" height="h-6" />
	{:else if !catalog || !routeData}
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
					{#if vehicle?.vehicleLabel}
						<span class="shrink-0 text-xs text-muted tabular-nums">
							{vehicle.vehicleLabel}
						</span>
					{/if}
				</div>

				{#if summary}
					<div class="flex items-center justify-between gap-2 text-sm">
						<span class="truncate text-secondary">
							{summary.description}
						</span>
						{#if summary.etaLabel}
							<span
								class="shrink-0 rounded-none bg-surface-2 px-1.5 py-0.5 text-xs font-semibold text-primary tabular-nums"
							>
								{summary.etaLabel}
							</span>
						{/if}
					</div>

					{#if vehicle?.deviationText || (isBus && read?.trip.status)}
						<div class="flex items-center gap-2 text-xs text-muted">
							{#if vehicle?.deviationText}
								<span
									class={vehicle.deviationTone === 'warning'
										? 'text-status-warning'
										: 'text-secondary'}
								>
									{vehicle.deviationText}
								</span>
							{/if}

							{#if isBus && read?.trip.status}
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
