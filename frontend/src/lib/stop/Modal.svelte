<script lang="ts">
	import Button from '#lib/Button.svelte';
	import type { CatalogStop, Source } from '#lib/client/index.js';
	import { stopArrivals } from '#lib/data/arrivals.js';
	import { upcomingCalls } from '#lib/data/decode.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import ModalList from '#lib/ModalList.svelte';
	import ModalListSkeleton from '#lib/ModalListSkeleton.svelte';
	import Icon from '#lib/route/Icon.svelte';
	import BusArrow from '#lib/stop/BusArrow.svelte';
	import Transfers from '#lib/stop/Transfers.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { getCurrentTime } from '#lib/url-params.svelte.js';
	import { mainRouteStops, tripHeadsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { CircleAlert } from '@lucide/svelte';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		showPrevious: boolean;
		timeFormat: 'time' | 'countdown';
		source: Source;
		stopId: string;
	}

	let { source, stopId, showPrevious, timeFormat }: Props = $props();

	const at = $derived(currentTime.value);
	const catalogResource = transit.catalog(() => source);
	const arrivalsResource = transit.stopArrivals(() => ({ source, stopId, at }));
	const alertsResource = transit.alerts(() => ({ source, at }));

	let selectedDirection = new LocalStorage<Partial<Record<Source, number>>>('selectedDirection', {
		mta_subway: 1,
		mta_bus: 0,
		njt_bus: 0
	});

	const alertsRead = $derived(alertsResource.current);
	const showAlertIcon = $derived(
		alertsRead.state === 'ready' &&
			alertsRead.value.alerts.some((alert) => alert.entities.some((e) => e.stop_id === stopId))
	);

	const catalogRead = $derived(catalogResource.current);
	const catalog = $derived(catalogRead.state === 'ready' ? catalogRead.value : undefined);
	const stop = $derived(catalog?.stopsById.get(stopId));
	const arrivalsRead = $derived(arrivalsResource.current);
	const read = $derived(arrivalsRead.state === 'ready' ? arrivalsRead.value : undefined);
	const predictions = $derived.by(() => {
		const now = currentTime.ms;
		const calls = showPrevious ? (read?.calls ?? []) : upcomingCalls(read?.calls ?? [], now);
		return stopArrivals(calls, read?.routes ?? [], now, { showPrevious: true });
	});
	const visibleArrivals = $derived(
		source === 'mta_subway'
			? predictions.arrivals.filter(
					(call) => call.trip.direction === selectedDirection.current[source]
				)
			: predictions.arrivals
	);
	const badges = $derived(stop ? routeBadges(stop, predictions.activeRoutes) : []);

	// More than six routes: main ones first, then active before inactive, then by ID length.
	function routeBadges(stop: CatalogStop, activeRoutes: ReadonlySet<string>) {
		const main = mainRouteStops(stop.routes, activeRoutes);
		if (main.length < 6) return main;
		return main.sort((a, b) => {
			const aActive = activeRoutes.has(a.route_id);
			const bActive = activeRoutes.has(b.route_id);
			if (aActive !== bActive) return aActive ? -1 : 1;
			return a.route_id.length - b.route_id.length;
		});
	}
</script>

<div class="flex flex-col items-start gap-3 border-b border-border-default p-3">
	<div class="flex w-full items-center gap-2">
		{#if stop?.data.source === 'mta_bus'}
			<BusArrow direction={stop.data.direction} />
		{/if}
		<div class="min-w-0">
			<h2 class="text-lg leading-snug font-semibold">{stop?.name ?? `Stop ${stopId}`}</h2>
		</div>

		{#if showAlertIcon}
			<CircleAlert size="1.5rem" class="text-red-800" />
		{/if}
	</div>
	<div class="flex flex-wrap items-center gap-2">
		{#each badges.length > 6 ? badges.slice(0, 5) : badges as routeStop (routeStop.route_id)}
			{const route = $derived(catalog?.routesById.get(routeStop.route_id))}
			{#if route}
				<Icon width={28} height={28} link={true} {route} showAlerts />
			{/if}
		{/each}
		{#if badges.length > 6}
			<div class="rounded-none bg-border-default p-1 font-semibold">+{badges.length - 5}</div>
		{/if}
	</div>
</div>
{#if stop}
	{#key `${source}:${stop.id}`}<Transfers {stop} {timeFormat} />{/key}
{/if}
{#if catalogRead.state === 'loading'}
	<ModalListSkeleton />
{:else if catalog && !stop}
	<p class="p-2 text-center text-muted">Stop not found</p>
{:else if stop}
	{#if !visibleArrivals.length && arrivalsRead.state === 'loading'}
		<ModalListSkeleton />
	{:else if !visibleArrivals.length}
		{#if arrivalsRead.state === 'ready'}
			<div class="px-4 py-8 text-center text-base font-semibold text-muted">
				No predictions to show
			</div>
		{/if}
	{:else}
		<ModalList>
			{#each visibleArrivals as st (`${st.tripId}:${st.sequence}`)}
				{const route = $derived(catalog?.routesById.get(st.routeId))}
				{const status = $derived(st.trip.status)}
				<Button state={{ type: 'trip', source, routeId: st.routeId, id: st.tripId }}>
					<div class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-1">
						{#if route}
							<Icon
								width={source === 'mta_subway' ? 20 : 32}
								height={source === 'mta_subway' ? 20 : 32}
								link={false}
								{route}
								class="col-start-1 row-start-1 justify-self-center"
							/>
						{/if}
						{#if status?.data.source === 'mta_bus' || status?.data.source === 'njt_bus'}
							<div class="col-start-1 row-start-2 flex justify-self-center">
								<VehicleCapacity {status} />
							</div>
						{/if}

						<div
							class={[
								'col-start-2 row-start-1 text-left',
								{ 'text-muted': st.arrival < currentTime.ms }
							]}
						>
							{tripHeadsign(
								st.trip,
								route,
								read?.routes.get(st.routeId)?.callsByTrip.get(st.tripId),
								catalog?.stopsById
							)}
						</div>
					</div>

					<div
						class={[
							'flex flex-col items-end',
							{
								italic: status?.data.source === 'mta_subway' && !status.data.assigned,
								'text-muted': st.arrival < currentTime.ms
							}
						]}
					>
						<div>
							{#if timeFormat === 'time'}
								{new Date(st.arrival).toLocaleTimeString().replace(/AM|PM/, '')}
							{:else}
								{st.eta.toFixed(0)}m
							{/if}
						</div>
					</div>
				</Button>
			{/each}
		</ModalList>
		{#if read?.pending.length}
			<ModalListSkeleton rows={1} />
		{/if}
	{/if}
{/if}

{#if stop?.data.source === 'mta_subway'}
	<div
		class="grid grid-cols-2 border-border-default bg-surface-2 text-primary"
		aria-label="Trip information"
	>
		{#each [{ direction: 1, name: stop.data.north_headsign }, { direction: 3, name: stop.data.south_headsign }] as tab (tab.direction)}
			<button
				class={[
					'p-2',
					{
						'bg-accent-bg text-accent': selectedDirection.current[source] === tab.direction,
						'text-muted': selectedDirection.current[source] !== tab.direction
					}
				]}
				onclick={() => {
					selectedDirection.current = { ...selectedDirection.current, [source]: tab.direction };
				}}>{tab.name}</button
			>
		{/each}
	</div>
{/if}
