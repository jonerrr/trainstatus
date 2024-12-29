<script lang="ts">
	import { ArrowBigRight, ChevronDown, ChevronUp } from 'lucide-svelte';
	import { page } from '$app/state';
	import { slide } from 'svelte/transition';
	import { stop_times as rt_stop_times } from '$lib/stop_times.svelte';
	import { current_time } from '$lib/util.svelte';
	import type { Stop } from '$lib/static';
	import {
		is_bus_route,
		is_train_route,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';
	import Transfers from './Transfers.svelte';

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		trip: Trip<TrainTripData | BusTripData>;
	}

	const { trip, show_previous, time_format }: Props = $props();

	const route = $derived(page.data.routes[trip.route_id]);

	const stop_times = $derived(rt_stop_times.stop_times.filter((st) => st.trip_id === trip.id)!);
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

	interface OpenTransfers {
		[stop_id: number]: boolean;
	}

	// Should open transfers be reset when changing trips?
	const open_transfers = $state<OpenTransfers>({});
</script>

<div class="flex gap-1 items-center p-1">
	<div class="flex flex-col gap-1 items-center">
		{#if is_bus_route(route, trip) && trip.data.passengers && trip.data.capacity}
			<BusCapacity passengers={trip.data.passengers} capacity={trip.data.capacity} />
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
		<div class="text-xs {trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}">
			{(trip.data.deviation / 60).toFixed(0)}m
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
		<div class="relative text-base">
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
				class="bg-neutral-800 z-20 absolute left-0 top-[50%] -translate-y-1/2 h-[95%] rounded"
			>
				<div class="flex items-center mx-1">
					<!-- Transfers -->
					{#if !open_transfers[st.stop_id]}
						<ChevronDown />
					{:else}
						<ChevronUp />
					{/if}
				</div>
			</button>

			<Button state={{ modal: 'stop', data: stop }}>
				<div class="flex flex-col w-full">
					<div class="flex items-center justify-between">
						<div class="text-left pl-10">
							{stop.name}
						</div>

						<div class="flex gap-1 items-center text-right">
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
				<Transfers stop_time={st} {trip} {time_format} />
			</div>
		{/if}
	{/each}
</ModalList>
