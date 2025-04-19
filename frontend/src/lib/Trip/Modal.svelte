<script lang="ts">
	import { ArrowBigRight, ChevronDown, ChevronUp } from '@lucide/svelte';
	import { page } from '$app/state';
	import { slide } from 'svelte/transition';
	import { stop_times as rt_stop_times, type StopTime } from '$lib/stop_times.svelte';
	import { current_time } from '$lib/util.svelte';
	import { is_train, type Stop } from '$lib/static';
	import { is_bus_route, is_train_route, trips, type Trip, type TripData } from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';
	import Transfers from './Transfers.svelte';

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		trip: Trip<TripData>;
	}

	const { trip, show_previous, time_format }: Props = $props();

	const route = $derived(page.data.routes[trip.route_id]);

	const stop_times = $derived(
		(rt_stop_times.by_trip_id[trip.id] || []).filter(
			(st) =>
				st.arrival.getTime() > current_time.ms ||
				show_previous ||
				page.url.pathname.startsWith('/charts') // charts only show trips that have already passed
		)
	);
	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		if (is_bus_route(route, trip)) {
			// TODO: get actual last stop instead of headsign
			// get stop in the direction of trip and get headsign
			const stop = page.data.stops[stop_times[0].stop_id] as Stop<'bus'>;
			return stop.routes.find((r) => r.id === route.id)!.headsign;
		} else {
			const last_st = stop_times[stop_times.length - 1];
			return page.data.stops[last_st.stop_id].name;
		}
	});

	interface StopTransfers {
		[stop_id: number]: StopTime<Trip>[];
	}

	const transfer_stop_times = $derived.by(() => {
		const transfers: StopTransfers = {};
		// TODO: maybe only get stop_times that are in the future
		for (const st of stop_times) {
			transfers[st.stop_id] = [];

			// only show 1 transfer for each route
			const added_routes = new Set<string>();

			const stop = page.data.stops[st.stop_id];
			const transfer_stop_times = rt_stop_times.by_stop_id[stop.id] || [];

			for (let transfer_st of transfer_stop_times) {
				if (transfers[st.stop_id].length > 3) break;
				if (transfer_st.trip_id === st.trip_id || transfer_st.arrival < st.arrival) continue;

				const transfer_trip = trips.trips.get(transfer_st.trip_id);
				if (
					!transfer_trip ||
					transfer_trip.route_id === trip.route_id ||
					transfer_trip.direction !== trip.direction ||
					added_routes.has(transfer_trip.route_id)
				)
					continue;

				// need to snapshot transfer so theres no unsafe state mutation error
				transfer_st = $state.snapshot(transfer_st);
				transfer_st.trip = transfer_trip;
				transfers[st.stop_id].push(transfer_st);
				added_routes.add(transfer_trip.route_id);
			}

			// only train routes have stop.transfers
			if (is_train(stop) && stop.data.transfers.length) {
				for (const transfer of stop.data.transfers) {
					if (transfers[st.stop_id].length > 3) break;

					const transfer_stop_times = rt_stop_times.by_stop_id[transfer] || [];
					for (let transfer_st of transfer_stop_times) {
						if (transfer_st.arrival < st.arrival) continue;
						const transfer_trip = trips.trips.get(transfer_st.trip_id);

						// TODO: maybe don't check direction bc it be different for other stops
						if (
							!transfer_trip ||
							transfer_trip.direction !== trip.direction ||
							added_routes.has(transfer_trip.route_id)
						)
							continue;

						// need to snapshot transfer so theres no unsafe state mutation error
						transfer_st = $state.snapshot(transfer_st);
						transfer_st.trip = transfer_trip;
						transfers[st.stop_id].push(transfer_st);
						added_routes.add(transfer_trip.route_id);
					}
				}
			}
		}

		// sort transfers by arrival time
		for (const stop_id in transfers) {
			transfers[stop_id].sort((a, b) => a.arrival.getTime() - b.arrival.getTime());
		}

		return transfers;
	});

	interface OpenTransfers {
		[stop_id: number]: boolean;
	}

	// Should open transfers be reset when changing trips?
	const open_transfers = $state<OpenTransfers>({});
</script>

<div class="flex items-center gap-1 p-1">
	<div class="flex flex-col items-start gap-1">
		{#if is_bus_route(route, trip)}
			{#if trip.data.passengers && trip.data.capacity}
				<BusCapacity passengers={trip.data.passengers} capacity={trip.data.capacity} />
			{/if}
			<div>#{trip.vehicle_id}</div>
		{/if}

		<Icon
			width={36}
			height={36}
			express={is_train_route(route, trip) && trip.data.express}
			{route}
			link
			show_alerts
		/>
	</div>

	<ArrowBigRight class="w-8" />

	<div class="text-xl font-semibold">
		{last_stop}
	</div>

	{#if is_bus_route(route, trip) && trip.data.deviation && Math.abs(trip.data.deviation) > 120}
		<div class="ml-auto text-sm {trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}">
			{trip.data.deviation > 0 ? '+' : ''}{(trip.data.deviation / 60).toFixed(0)}m
		</div>
	{/if}
</div>

<!-- <div class="text-left">
		{stop.name}
		{#if stop_time.stop_id === stop_id}
			<span class="text-indigo-400 text-xs">
				{#if train_status === TrainStatus.AtStop}
					(at stop)
				{:else if train_status === TrainStatus.InTransitTo}
					(approaching)
				{:else if train_status === TrainStatus.Incoming}
					(arriving)
				{/if}
			</span>
		{/if}
	</div>
	{#if stop_time.arrival > new Date()}
		<div class={`text-right`}>
			{stop_time.arrival.toLocaleTimeString()}
		</div>
	{:else}
		<div class={`text-right text-neutral-400`}>
			{stop_time.arrival.toLocaleTimeString()}
		</div>
	{/if} -->

<ModalList>
	{#each stop_times as st}
		{@const stop = page.data.stops[st.stop_id]}
		<!-- {#if rt_stop_times.by_stop_id[]} -->
		<div class="relative text-base">
			{#if transfer_stop_times[st.stop_id].length}
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
						<!-- Transfers -->
						{#if !open_transfers[st.stop_id]}
							<ChevronDown />
						{:else}
							<ChevronUp />
						{/if}
					</div>
				</button>
			{/if}

			<Button state={{ modal: 'stop', data: stop }}>
				<div
					class="flex w-full flex-col"
					class:text-neutral-400={st.arrival.getTime() < current_time.ms}
				>
					<div class="flex items-center justify-between">
						<div class="pl-10 text-left">
							{stop.name}
						</div>

						<!-- TODO: maybe italicize if trip isn't assigned -->
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
	{/each}
</ModalList>
