<script lang="ts">
	import { slide } from 'svelte/transition';

	import { page } from '$app/state';

	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Skeleton from '$lib/Skeleton.svelte';
	import Transfers from '$lib/Trip/Transfers.svelte';
	import VehicleCapacity from '$lib/VehicleCapacity.svelte';
	import type { StopTime, Trip } from '$lib/client';
	import { position_context } from '$lib/resources/positions.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { current_time } from '$lib/url_params.svelte';

	import { ArrowBigRight, ChevronDown, ChevronUp } from '@lucide/svelte';

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		trip: Trip;
	}

	const { trip, show_previous, time_format }: Props = $props();

	const route = $derived(page.data.routes_by_id[trip.data.source]?.[trip.route_id]);

	const all_trips = trip_context.get();
	const all_stop_times = stop_time_context.get();

	const source_stop_times = $derived(all_stop_times[trip.data.source]);

	const all_trip_stop_times = $derived(source_stop_times?.current.by_trip_id.get(trip.id) ?? []);

	const st_loading = $derived(
		!source_stop_times || (source_stop_times.status !== 'ready' && all_trip_stop_times.length === 0)
	);

	const stop_times = $derived(
		all_trip_stop_times.filter(
			(st) =>
				st.arrival.getTime() > current_time.ms ||
				show_previous ||
				page.url.pathname.startsWith('/charts') // charts only show trips that have already passed TODO: maybe remove this
		)
	);

	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		switch (trip.data.source) {
			case 'mta_bus':
				const stop = page.data.stops_by_id[trip.data.source]?.[stop_times[0].stop_id];
				const routeStop = stop?.routes.find((r) => r.route_id === trip.route_id);
				if (!routeStop) return 'Unknown';
				// this shouldn't be necessary since we should only be looking at bus routes, but just in case (and also to satisfy type checker)
				return routeStop.data.source === 'mta_bus' ? routeStop.data.headsign : 'Unknown';
			case 'mta_subway':
				const last_st = stop_times[stop_times.length - 1];
				return page.data.stops_by_id[trip.data.source]?.[last_st.stop_id]?.name ?? 'Unknown';
			default:
				return 'Unknown';
		}
	});

	type StopTransfers = Record<string, StopTime[]>;

	const transfer_stop_times = $derived.by(() => {
		const transfers: StopTransfers = {};
		for (const st of stop_times) {
			transfers[st.stop_id] = [];

			const added_routes = new Set<string>();

			const stop = page.data.stops_by_id[st.data.source]?.[st.stop_id];
			if (!stop) continue;

			const stop_st = all_stop_times[st.data.source]?.current.by_stop_id.get(stop.id) || [];
			for (const transfer_st of stop_st) {
				if (transfers[st.stop_id].length > 3) break;
				if (transfer_st.trip_id === st.trip_id || transfer_st.arrival < st.arrival) continue;

				const transfer_trip = all_trips[transfer_st.data.source]?.current?.get(transfer_st.trip_id);
				if (
					!transfer_trip ||
					transfer_trip.route_id === trip.route_id ||
					transfer_trip.direction !== trip.direction ||
					added_routes.has(transfer_trip.route_id)
				)
					continue;

				transfers[st.stop_id].push(transfer_st);
				added_routes.add(transfer_trip.route_id);
			}

			for (const transfer of stop.transfers) {
				if (transfers[st.stop_id].length > 3) break;

				const t_stop_st =
					all_stop_times[transfer.to_stop_source]?.current.by_stop_id.get(transfer.to_stop_id) ||
					[];
				for (const transfer_st of t_stop_st) {
					if (transfer_st.arrival < st.arrival) continue;
					const transfer_trip = all_trips[transfer_st.data.source]?.current?.get(
						transfer_st.trip_id
					);

					// TODO: maybe don't check direction bc it can be different for other stops
					// also now with the multi-source setup, each source has different direction numbers (e.g. 1&3 for subway, 0&1 for bus), so we should check direction within the same source but not across sources

					if (
						!transfer_trip ||
						(transfer_trip.data.source === trip.data.source &&
							transfer_trip.direction !== trip.direction) ||
						added_routes.has(transfer_trip.route_id)
					)
						continue;

					transfers[st.stop_id].push(transfer_st);
					added_routes.add(transfer_trip.route_id);
				}
			}
		}

		for (const stop_id in transfers) {
			transfers[stop_id].sort((a, b) => a.arrival.getTime() - b.arrival.getTime());
		}

		return transfers;
	});

	type OpenTransfers = Record<string, boolean>;

	const open_transfers = $state<OpenTransfers>({});
</script>

<div class="flex items-center gap-1 p-1">
	<div class="flex flex-col items-start gap-1">
		{#if trip.data.source === 'mta_bus'}
			{@const position = position_context
				.getSource(trip.data.source)
				?.current?.get(trip.vehicle_id)}
			<VehicleCapacity {position} />
			<div>#{trip.vehicle_id}</div>
		{/if}

		{#if route}
			<Icon width={36} height={36} {route} link show_alerts />
		{/if}
	</div>

	<ArrowBigRight class="w-8" />

	<div class="text-xl font-semibold">
		{last_stop}
	</div>

	{#if trip.data.source === 'mta_bus' && trip.data.deviation && Math.abs(trip.data.deviation) > 120}
		<div class="ml-auto text-sm {trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}">
			{trip.data.deviation > 0 ? '+' : ''}{(trip.data.deviation / 60).toFixed(0)}m
		</div>
	{/if}
</div>

{#if st_loading}
	<Skeleton lines={6} class="p-2" />
{:else}
	<ModalList>
		{#each stop_times as st}
			{@const stop = page.data.stops_by_id[st.data.source]?.[st.stop_id]}
			{#if stop}
				<div class="relative text-base">
					{#if transfer_stop_times[st.stop_id]?.length}
						<button
							tabindex="0"
							onclick={() => {
								if (open_transfers[st.stop_id]) {
									open_transfers[st.stop_id] = false;
								} else {
									open_transfers[st.stop_id] = true;
								}
							}}
							aria-label="Show transfers at stop"
							class="absolute top-[50%] left-0 z-20 h-[95%] -translate-y-1/2 rounded-sm bg-neutral-800"
						>
							<div class="mx-1 flex items-center">
								{#if !open_transfers[st.stop_id]}
									<ChevronDown />
								{:else}
									<ChevronUp />
								{/if}
							</div>
						</button>
					{/if}

					<Button state={{ type: 'stop', ...stop }}>
						<div
							class="flex w-full flex-col"
							class:text-neutral-400={st.arrival.getTime() < current_time.ms}
						>
							<div class="flex items-center justify-between">
								<div class="pl-10 text-left">
									{stop.name}
								</div>

								<div class="flex items-center gap-1 text-right">
									<div class="text-left">
										{#if time_format === 'time'}
											{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
										{:else}
											{((st.arrival.getTime() - current_time.ms) / 1000 / 60).toFixed(0)}m
										{/if}
									</div>
								</div>
							</div>
						</div>
					</Button>
				</div>

				{#if open_transfers[st.stop_id]}
					<div transition:slide>
						<Transfers {time_format} transfer_stop_times={transfer_stop_times[st.stop_id]} />
					</div>
				{/if}
			{/if}
		{/each}
	</ModalList>
{/if}
