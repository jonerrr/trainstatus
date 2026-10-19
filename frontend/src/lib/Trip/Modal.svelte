<script lang="ts">
	import { page } from '$app/state';

	import Button from '#lib/Button.svelte';
	import type { Source } from '#lib/client/index.js';
	import ModalList from '#lib/ModalList.svelte';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { boardingOptions } from '#lib/transit/nearby.js';
	import { getTransitData, type TripRead } from '#lib/transit/store.svelte.js';
	import { arrivalExits } from '#lib/Trip/platform-exits.js';
	import PlatformExits from '#lib/Trip/PlatformExits.svelte';
	import Transfers from '#lib/Trip/Transfers.svelte';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { trip_destination_title, trip_headsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { ChevronDown, ChevronUp, Circle } from '@lucide/svelte';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		source: Source;
		route_id: string;
		trip_id: string;
	}

	const { source, route_id, trip_id, show_previous, time_format }: Props = $props();

	const at = $derived(current_time.value);
	const catalog_resource = transit.catalog(() => source);
	const trip_resource = transit.trip(() => ({ source, route_id, trip_id, at }));
	// The header stays mounted across loading states and shows whatever has loaded.
	const loaded_catalog = $derived(transit.peekCatalog(source));
	const route = $derived(loaded_catalog?.routes_by_id.get(route_id));
	const loaded_read = $derived(trip_resource.current.value);
	let open_transfer = $state<string | null>(null);
	let open_exits = $state<string | null>(null);
	const transfer_catalogs_resource = transit.catalogs(() => page.data.selected_sources);
	const transfer_catalogs = $derived(
		transfer_catalogs_resource.current.flatMap((r) => (r.catalog ? [r.catalog] : []))
	);

	function visible_calls(read: TripRead) {
		const now = current_time.ms;
		// Charts only show trips that have already passed.
		if (show_previous || page.url.pathname.startsWith('/charts')) return read.calls;
		return read.calls.filter((call) => call.arrival > now);
	}
</script>

{#snippet title_row(read: TripRead | undefined)}
	{const status = $derived(read?.trip.status)}
	{const is_bus = $derived(source === 'mta_bus' || source === 'njt_bus')}
	{const trip_data = $derived(read?.trip.data)}
	{const deviation = $derived(
		trip_data?.source === 'mta_bus' && trip_data.deviation && Math.abs(trip_data.deviation) > 120
			? trip_data.deviation / 60
			: undefined
	)}
	<div class="border-b border-neutral-700 p-3">
		<div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-x-3">
			{#if route}
				<Icon
					{route}
					width={28}
					height={28}
					link
					show_alerts
					class="col-start-1 row-start-1 self-center justify-self-center"
				/>
			{/if}
			{#if is_bus && read}
				<div
					class={[
						'flex h-5 items-center justify-self-center leading-5',
						route ? 'col-start-1 row-start-2' : 'col-start-1 row-start-1'
					]}
				>
					<VehicleCapacity {status} />
				</div>
				<span
					class="col-start-3 row-start-1 flex min-h-7 shrink-0 items-center self-center text-xs leading-4 text-neutral-400"
					>Bus #{read.trip.vehicle_id}</span
				>
			{/if}
			<div class="col-start-2 row-start-1 flex min-h-7 min-w-0 items-center self-center text-left">
				<div class="text-sm leading-4 font-semibold wrap-anywhere text-neutral-100">
					{#if read}
						{trip_destination_title(
							trip_headsign(read.trip, route, read.calls, loaded_catalog?.stops_by_id),
							route
						)}
					{:else}
						{route?.long_name ?? ''}
					{/if}
				</div>
			</div>
			{#if deviation !== undefined}
				<div
					class="col-start-2 row-start-2 flex h-5 items-center text-sm leading-5 text-neutral-300"
				>
					{Math.abs(deviation).toFixed(0)} min {deviation > 0 ? 'late' : 'early'}
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet consist(read: TripRead)}
	{const data = $derived(read.trip.data)}
	{const subway_consist = $derived(data.source === 'mta_subway' ? data.consist : null)}
	{const cars = $derived(data.source === 'mta_subway' ? (data.consist_cars ?? []) : [])}
	{const model = $derived(cars[0]?.type)}
	<details
		class="group mx-2 mb-2 overflow-hidden rounded-xl border border-neutral-700/80 bg-neutral-900/80 text-sm shadow-sm shadow-black/20"
	>
		<summary
			class="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-neutral-100 transition hover:bg-neutral-800/60 [&::-webkit-details-marker]:hidden"
		>
			<div class="flex min-w-0 flex-1 items-center gap-2">
				<div class="text-xs font-semibold tracking-wide text-neutral-400 uppercase">Consist</div>

				{#if subway_consist}
					<div class="flex min-w-0 items-center gap-1.5 text-xs text-neutral-300">
						<span class="truncate">{subway_consist.car_count} cars</span>
						{#if model}
							<Circle class="h-1 w-1 shrink-0 fill-current text-neutral-500" />
							<span class="truncate">{model}</span>
						{/if}
					</div>
				{:else}
					<div class="truncate text-xs text-neutral-400">No consist details available</div>
				{/if}
			</div>

			<ChevronDown class="h-4 w-4 shrink-0 text-neutral-400 transition group-open:rotate-180" />
		</summary>

		<div class="border-t border-neutral-800 px-3 pt-2 pb-3">
			{#if subway_consist}
				{#if cars.length}
					<div class="overflow-x-auto pb-1">
						<div class="mx-auto flex w-fit min-w-max items-center justify-center gap-px">
							{#each cars as car, index (car.number)}
								<div
									class={[
										'flex min-h-7 items-center justify-center border border-neutral-600 bg-neutral-800 px-2.5 text-center text-[0.7rem] leading-none shadow-inner shadow-white/5',
										index === 0 && 'rounded-l-full pl-3.5',
										index === cars.length - 1 && 'rounded-r-full pr-3.5',
										index !== 0 && 'border-l-0'
									]}
									title={car.number}
								>
									<div class="font-medium text-neutral-100">#{car.number}</div>
								</div>
							{/each}
						</div>
					</div>
				{:else}
					<div class="text-xs text-neutral-400">No cars are listed for this train yet.</div>
				{/if}
			{:else}
				<div class="text-xs text-neutral-400">
					No consist details are available for this train yet.
				</div>
			{/if}
		</div>
	</details>
{/snippet}

{@render title_row(loaded_read)}
<svelte:boundary>
	{const catalog_state = $derived(catalog_resource.current)}
	{const trip_state = $derived(trip_resource.current)}
	{const catalog = $derived(catalog_state.value)}
	{const read = $derived(trip_state.value)}

	{#if catalog_state.loading || trip_state.loading}
		<Skeleton lines={6} class="p-2" />
	{:else if !read && !catalog_state.error && !trip_state.error}
		<p class="p-2 text-center text-neutral-400">This trip is not currently running.</p>
	{:else if read}
		{#if source === 'mta_subway'}
			{@render consist(read)}
		{/if}

		{const calls = $derived(visible_calls(read))}
		<ModalList>
			{#each calls as st (st.sequence)}
				{const stop = $derived(catalog?.stops_by_id.get(st.stop_id))}
				{#if stop}
					{const incoming = $derived({
						source,
						route_id,
						arrival: st.arrival
					})}
					{const nearby_options = $derived(boardingOptions(stop, transfer_catalogs, incoming))}
					{const transfer_key = JSON.stringify([source, route_id, trip_id, st.sequence])}
					{const exits = $derived(arrivalExits(stop, read.trip, st.platform_edges))}
					<div class="relative text-base">
						{#if nearby_options.length}
							<button
								tabindex="0"
								onclick={() => {
									open_transfer = open_transfer === transfer_key ? null : transfer_key;
								}}
								aria-label={`Transfers at ${stop.name}: nearby routes`}
								aria-expanded={open_transfer === transfer_key}
								class="absolute top-[50%] left-0 z-20 h-[95%] -translate-y-1/2 rounded-sm bg-neutral-800"
							>
								<div class="mx-1 flex min-w-6 flex-col items-center">
									{#if open_transfer !== transfer_key}
										<ChevronDown size={18} />
									{:else}
										<ChevronUp size={18} />
									{/if}
								</div>
							</button>
						{/if}

						<Button state={{ type: 'stop', source, id: stop.id }}>
							<div
								class={[
									'flex w-full flex-col',
									{ 'text-neutral-400': st.arrival < current_time.ms }
								]}
							>
								<div class="flex items-center justify-between">
									<div class="pl-10 text-left">
										{stop.name}
									</div>

									<div class="flex items-center gap-1 text-right">
										<div class="text-left">
											{#if time_format === 'time'}
												{new Date(st.arrival).toLocaleTimeString().replace(/AM|PM/, '')}
											{:else}
												{((st.arrival - current_time.ms) / 1000 / 60).toFixed(0)}m
											{/if}
										</div>
									</div>
								</div>
							</div>
						</Button>
					</div>

					{#if exits}
						<button
							type="button"
							class="mb-1 ml-12 flex min-h-9 items-center gap-1 rounded px-2 text-xs text-sky-300 hover:bg-neutral-800"
							aria-label={`Where to sit for ${stop.name}`}
							aria-expanded={open_exits === transfer_key}
							onclick={() => {
								open_exits = open_exits === transfer_key ? null : transfer_key;
							}}
						>
							Where to sit
							<ChevronDown size={14} class={open_exits === transfer_key ? 'rotate-180' : ''} />
						</button>
						{#if open_exits === transfer_key}
							{#key transfer_key}<PlatformExits guidance={exits} />{/key}
						{/if}
					{/if}

					{#if open_transfer === transfer_key}
						{#key transfer_key}<Transfers {time_format} {stop} {incoming} />{/key}
					{/if}
				{/if}
			{:else}
				<p class="p-2 text-center text-neutral-400">No predictions to show</p>
			{/each}
		</ModalList>
	{/if}
</svelte:boundary>
