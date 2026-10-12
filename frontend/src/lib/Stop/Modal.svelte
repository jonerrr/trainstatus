<script lang="ts">
	import Button from '#lib/Button.svelte';
	import type { CatalogStop, Source } from '#lib/client/index.js';
	import ModalList from '#lib/ModalList.svelte';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import BusArrow from '#lib/Stop/BusArrow.svelte';
	import Transfers from '#lib/Stop/Transfers.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { isStale, upcomingCalls, type CatalogData } from '#lib/transit/decode.js';
	import { getTransitData, loaded, type StopArrivals } from '#lib/transit/store.svelte.js';
	import { stop_arrivals } from '#lib/transit/view.js';
	import UnavailableRows from '#lib/UnavailableRows.svelte';
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

	const at = $derived(current_time.value ?? null);

	let selected_direction = new LocalStorage<Partial<Record<Source, number>>>('selectedDirection', {
		mta_subway: 1,
		mta_bus: 0,
		njt_bus: 0
	});

	const show_alert_icon = $derived(
		loaded(transit.alerts(source, at))?.alerts.some((alert) =>
			alert.entities.some((e) => e.stop_id === stop_id)
		)
	);

	function rows(read: StopArrivals) {
		const now = current_time.ms;
		const routes = [...read.routes.values()];
		const stale = routes.some((route) => isStale(route, now));
		const { calls, unavailable } = show_previous
			? { calls: read.calls, unavailable: false }
			: upcomingCalls(read.calls, now, stale);
		const { arrivals, active_routes } = stop_arrivals(calls, read.routes, now, {
			show_previous: true
		});
		const direction = selected_direction.current[source];
		const fetched = routes.map((route) => route.fetched_at ?? 0);
		return {
			arrivals:
				source === 'mta_subway'
					? arrivals.filter((st) => st.trip.direction === direction)
					: arrivals,
			active_routes,
			unavailable: unavailable || (!routes.length && read.failed.length > 0),
			fetched_at: fetched.length ? Math.min(...fetched) : null
		};
	}

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

{#snippet header(stop: CatalogStop | undefined, catalog: CatalogData | undefined)}
	{const active_routes = $derived(
		new Set(
			transit.loadedStopArrivals(source, stop_id, at).calls.map((call) => call.route_id)
		) as ReadonlySet<string>
	)}
	{const badges = $derived(stop ? route_badges(stop, active_routes) : [])}
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
{/snippet}

{#snippet direction_tab(direction: number, name: string)}
	<button
		class={[
			'p-2',
			{
				'bg-neutral-900': selected_direction.current[source] === direction,
				'text-neutral-400': selected_direction.current[source] !== direction
			}
		]}
		onclick={() => {
			selected_direction.current[source] = direction;
		}}
	>
		{name}
	</button>
{/snippet}

<svelte:boundary>
	{const catalog = $derived(await transit.catalog(source))}
	{const stop = $derived(catalog.stops_by_id.get(stop_id))}
	{@render header(stop, catalog)}

	{#if !stop}
		<p class="p-2 text-center text-neutral-400">This stop is not in the saved stop list.</p>
	{:else}
		{#if stop.transfers.length}
			<Transfers stop_source={source} transfers={stop.transfers} />
		{/if}

		<svelte:boundary>
			{const read = $derived(await transit.stopArrivals(source, stop_id, at))}
			{const view = $derived(rows(read))}
			{#if view.unavailable}
				<UnavailableRows arrivals fetched_at={view.fetched_at} />
			{:else if !view.arrivals.length}
				{#if read.pending.length}
					<Skeleton lines={5} class="p-2" />
				{:else}
					<div class="text-center font-semibold text-neutral-400">No upcoming trips</div>
				{/if}
			{:else}
				<ModalList>
					{#each view.arrivals as st (`${st.trip_id}:${st.sequence}`)}
						{const route = $derived(catalog.routes_by_id.get(st.route_id))}
						{const status = $derived(st.trip.status)}
						<Button state={{ type: 'trip', source, route_id: st.route_id, id: st.trip_id }}>
							<div class="flex items-center gap-1">
								<div class="flex flex-col items-center">
									{#if status?.data.source === 'mta_bus' || status?.data.source === 'njt_bus'}
										<VehicleCapacity {status} />
									{/if}
									{#if route}
										<Icon
											width={source === 'mta_subway' ? 20 : 32}
											height={source === 'mta_subway' ? 20 : 32}
											link={false}
											{route}
										/>
									{/if}
								</div>

								<div class="text-left" class:text-neutral-400={st.arrival < current_time.ms}>
									{trip_headsign(
										st.trip,
										route,
										read.routes.get(st.route_id)?.calls_by_trip.get(st.trip_id),
										catalog.stops_by_id
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
				{#if read.pending.length}
					<Skeleton lines={1} class="p-2" />
				{/if}
			{/if}

			{#snippet pending()}
				<Skeleton lines={5} class="p-2" />
			{/snippet}
			{#snippet failed(_error, reset)}
				<UnavailableRows arrivals {reset} />
			{/snippet}
		</svelte:boundary>
	{/if}

	{#if stop?.data.source === 'mta_subway'}
		<div
			class="grid grid-cols-2 border-neutral-700 bg-neutral-800 text-neutral-100"
			aria-label="Trip information"
		>
			{@render direction_tab(1, stop.data.north_headsign)}
			{@render direction_tab(3, stop.data.south_headsign)}
		</div>
	{/if}

	{#snippet pending()}
		{@render header(undefined, undefined)}
		<Skeleton lines={5} class="p-2" />
	{/snippet}
	{#snippet failed(_error, reset)}
		{@render header(undefined, undefined)}
		<UnavailableRows arrivals {reset} />
	{/snippet}
</svelte:boundary>
