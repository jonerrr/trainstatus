<script lang="ts">
	import Button from '#lib/Button.svelte';
	import type { CatalogStop, Source } from '#lib/client/index.js';
	import ModalList from '#lib/ModalList.svelte';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import Transfers from '#lib/Stop/Transfers.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { stop_arrivals } from '#lib/transit/arrivals.js';
	import { upcomingCalls } from '#lib/transit/decode.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { main_route_stops, trip_headsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { CircleAlert } from '@lucide/svelte';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		source: Source;
		stop_id: string;
	}

	let { source, stop_id, show_previous, time_format }: Props = $props();

	const at = $derived(current_time.value);
	const catalog_resource = transit.catalog(() => source);
	const arrivals_resource = transit.stopArrivals(() => ({ source, stop_id, at }));
	const alerts_resource = transit.alerts(() => ({ source, at }));

	let selected_direction = new LocalStorage<Partial<Record<Source, number>>>('selectedDirection', {
		mta_subway: 1,
		mta_bus: 0,
		njt_bus: 0
	});

	const show_alert_icon = $derived(
		alerts_resource.current.value?.alerts.some((alert) =>
			alert.entities.some((e) => e.stop_id === stop_id)
		)
	);

	const catalog_state = $derived(catalog_resource.current);
	const catalog = $derived(catalog_state.value);
	const stop = $derived(catalog?.stops_by_id.get(stop_id));
	const arrivals_state = $derived(arrivals_resource.current);
	const read = $derived(arrivals_state.value);
	const predictions = $derived.by(() => {
		const now = current_time.ms;
		const calls = show_previous ? (read?.calls ?? []) : upcomingCalls(read?.calls ?? [], now);
		return stop_arrivals(calls, read?.routes ?? [], now, { show_previous: true });
	});
	const visible_arrivals = $derived(
		source === 'mta_subway'
			? predictions.arrivals.filter(
					(call) => call.trip.direction === selected_direction.current[source]
				)
			: predictions.arrivals
	);
	const badges = $derived(stop ? route_badges(stop, predictions.active_routes) : []);

	// More than six routes: main ones first, then active before inactive, then by ID length.
	function route_badges(stop: CatalogStop, active_routes: ReadonlySet<string>) {
		const main = main_route_stops(stop.routes, active_routes);
		if (main.length < 6) return main;
		return main.sort((a, b) => {
			const a_active = active_routes.has(a.route_id);
			const b_active = active_routes.has(b.route_id);
			if (a_active !== b_active) return a_active ? -1 : 1;
			return a.route_id.length - b.route_id.length;
		});
	}
</script>

<div class="flex flex-col items-start gap-3 p-3">
	<div class="flex w-full items-center gap-2">
		{#if stop?.data.source === 'mta_bus'}
			<BusArrow direction={stop.data.direction} />
		{/if}
		<div class="min-w-0">
			<h2 class="text-lg leading-snug font-semibold">{stop?.name ?? `Stop ${stop_id}`}</h2>
		</div>

		{#if show_alert_icon}
			<CircleAlert size="1.5rem" class="text-red-800" />
		{/if}
	</div>
	<div class="flex flex-wrap items-center gap-1">
		{#each badges.length > 6 ? badges.slice(0, 5) : badges as route_stop (route_stop.route_id)}
			{const route = $derived(catalog?.routes_by_id.get(route_stop.route_id))}
			{#if route}
				<Icon width={28} height={28} link={true} {route} show_alerts />
			{/if}
		{/each}
		{#if badges.length > 6}
			<div class="rounded-sm bg-neutral-700 p-1 font-semibold">+{badges.length - 5}</div>
		{/if}
	</div>
</div>
{#if catalog_state.loading}
	<Skeleton lines={5} class="p-2" />
{:else if stop}
	{#if stop.transfers.length}
		<Transfers stop_source={source} transfers={stop.transfers} />
	{/if}

	{#if arrivals_state.loading}
		<Skeleton lines={5} class="p-2" />
	{:else if !visible_arrivals.length}
		{#if read?.pending.length}
			<Skeleton lines={5} class="p-2" />
		{:else}
			<div class="px-4 py-8 text-center text-base font-semibold text-neutral-400">
				No predictions to show
			</div>
		{/if}
	{:else}
		<ModalList>
			{#each visible_arrivals as st (`${st.trip_id}:${st.sequence}`)}
				{const route = $derived(catalog?.routes_by_id.get(st.route_id))}
				{const status = $derived(st.trip.status)}
				<Button state={{ type: 'trip', source, route_id: st.route_id, id: st.trip_id }}>
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
								{ 'text-neutral-400': st.arrival < current_time.ms }
							]}
						>
							{trip_headsign(
								st.trip,
								route,
								read?.routes.get(st.route_id)?.calls_by_trip.get(st.trip_id),
								catalog?.stops_by_id
							)}
						</div>
					</div>

					<div
						class={[
							'flex flex-col items-end',
							{
								italic: status?.data.source === 'mta_subway' && !status.data.assigned,
								'text-neutral-400': st.arrival < current_time.ms
							}
						]}
					>
						<div>
							{#if time_format === 'time'}
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
			<Skeleton lines={1} class="p-2" />
		{/if}
	{/if}
{/if}

{#if stop?.data.source === 'mta_subway'}
	<div
		class="grid grid-cols-2 border-neutral-700 bg-neutral-800 text-neutral-100"
		aria-label="Trip information"
	>
		{#each [{ direction: 1, name: stop.data.north_headsign }, { direction: 3, name: stop.data.south_headsign }] as tab (tab.direction)}
			<button
				class={[
					'p-2',
					{
						'bg-neutral-900': selected_direction.current[source] === tab.direction,
						'text-neutral-400': selected_direction.current[source] !== tab.direction
					}
				]}
				onclick={() => {
					selected_direction.current = { ...selected_direction.current, [source]: tab.direction };
				}}>{tab.name}</button
			>
		{/each}
	</div>
{/if}
