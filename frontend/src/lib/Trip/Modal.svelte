<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import ModalList from '#lib/ModalList.svelte';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import type { Call } from '#lib/transit/decode.js';
	import { getTransitData, type TripRead } from '#lib/transit/store.svelte.js';
	import Transfers from '#lib/Trip/Transfers.svelte';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';
	import { trip_destination_title, trip_headsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { RotateCcw } from '@lucide/svelte';

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
	let selected_key = $state<string | null>(null);
	let transfers_open = $state(true);
	let stop_heights = $state<Record<number, number>>({});
	const calls = $derived(loaded_read ? visible_calls(loaded_read) : []);
	const stops_min_height = $derived(
		2 + calls.slice(0, 3).reduce((height, call) => height + (stop_heights[call.sequence] ?? 45), 0)
	);
	const selected_call = $derived(calls.find((call) => visitKey(call) === selected_key) ?? calls[0]);
	const selected_stop = $derived(
		selected_call ? loaded_catalog?.stops_by_id.get(selected_call.stop_id) : undefined
	);
	function visitKey(call: Call) {
		return JSON.stringify([source, route_id, trip_id, call.sequence]);
	}

	function visible_calls(read: TripRead) {
		const now = current_time.ms;
		const ordered = [...read.calls].sort((a, b) => a.sequence - b.sequence);
		// Charts only show trips that have already passed.
		if (show_previous || page.url.pathname.startsWith('/charts')) return ordered;
		return ordered.filter((call) => call.arrival > now);
	}
</script>

{#snippet title_row(read: TripRead | undefined)}
	{const status = $derived(read?.trip.status)}
	{const is_bus = $derived(source === 'mta_bus' || source === 'njt_bus')}
	{const trip_data = $derived(read?.trip.data)}
	{const model = $derived(
		trip_data?.source === 'mta_subway' ? trip_data.consist_cars?.[0]?.type : undefined
	)}
	{const car_count = $derived(
		trip_data?.source === 'mta_subway'
			? (trip_data.consist?.car_count ?? trip_data.consist_cars?.length)
			: undefined
	)}
	{const vehicle = $derived(
		[model, car_count ? `${car_count} ${car_count === 1 ? 'car' : 'cars'}` : undefined]
			.filter(Boolean)
			.join(' · ')
	)}
	{const deviation = $derived(
		trip_data?.source === 'mta_bus' && trip_data.deviation && Math.abs(trip_data.deviation) > 120
			? trip_data.deviation / 60
			: undefined
	)}
	<div class="shrink-0 border-b border-border-default p-3">
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
					class="col-start-3 row-start-1 flex min-h-7 shrink-0 items-center self-center text-xs leading-4 text-muted"
					>Bus #{read.trip.vehicle_id}</span
				>
			{/if}
			{#if vehicle}
				<span
					class="col-start-3 row-start-1 flex min-h-7 shrink-0 items-center self-center text-xs leading-4 text-muted tabular-nums"
					>{vehicle}</span
				>
			{/if}
			<div class="col-start-2 row-start-1 flex min-h-7 min-w-0 items-center self-center text-left">
				<div class="text-sm leading-4 font-semibold wrap-anywhere text-primary">
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
				<div class="col-start-2 row-start-2 flex h-5 items-center text-sm leading-5 text-secondary">
					{Math.abs(deviation).toFixed(0)} min {deviation > 0 ? 'late' : 'early'}
				</div>
			{/if}
		</div>
	</div>
{/snippet}

<div class="trip-content" style:--trip-stops-min-height={`${stops_min_height}px`}>
	{@render title_row(loaded_read)}
	<svelte:boundary>
		{const catalog_state = $derived(catalog_resource.current)}
		{const trip_state = $derived(trip_resource.current)}
		{const catalog = $derived(catalog_state.value)}
		{const read = $derived(trip_state.value)}

		{#if catalog_state.loading || trip_state.loading}
			<Skeleton lines={6} class="p-2" />
		{:else if !read && !catalog_state.error && !trip_state.error}
			<p class="p-2 text-center text-muted">This trip is not currently running.</p>
		{:else if read}
			{#if selected_call && selected_stop}
				<div class="preview-panel" aria-label="Selected stop preview" role="region">
					<div class="preview-title">
						<span>{selected_stop.name}</span><span class="preview-actions"
							>{#if selected_call !== calls[0]}<button
									type="button"
									onclick={() => (selected_key = null)}
									aria-label="Reset to next stop"
									title="Reset to next stop"><RotateCcw size={14} /></button
								>{/if}{@render arrival_time(selected_call)}</span
						>
					</div>
					{#key visitKey(selected_call)}
						<Transfers
							stop={selected_stop}
							incoming={{
								source,
								route_id,
								direction: read.trip.direction,
								arrival: selected_call.arrival
							}}
							{time_format}
							trip={read.trip}
							platform_edges={selected_call.platform_edges}
							bind:open={transfers_open}
						/>
					{/key}
				</div>
			{/if}
			<ModalList class="trip-stops">
				{#each calls as call (call.sequence)}
					{const stop = $derived(catalog?.stops_by_id.get(call.stop_id))}
					{#if stop}
						<div
							bind:offsetHeight={stop_heights[call.sequence]}
							class={[
								'stop-row',
								{ selected: selected_call === call, passed: call.arrival < current_time.ms }
							]}
						>
							<button
								type="button"
								class="select-stop"
								aria-label={`Preview ${stop.name}`}
								aria-pressed={selected_call === call}
								title={`Preview exits and transfers at ${stop.name}`}
								onclick={() => (selected_key = visitKey(call))}
								><span class="track" aria-hidden="true"
									><span class="dot relative size-3.5 rounded-none border-3 border-muted bg-canvas"
									></span></span
								></button
							>
							<button
								type="button"
								class="open-stop"
								aria-label={`Open ${stop.name} stop modal`}
								onclick={() => open_modal({ type: 'stop', source, id: stop.id })}
								><span>{stop.name}</span><span class="time">{@render arrival_time(call)}</span
								></button
							>
						</div>
					{/if}
				{:else}<p class="p-2 text-center text-muted">No predictions to show</p>{/each}
			</ModalList>
		{/if}
	</svelte:boundary>
</div>

{#snippet arrival_time(call: Call)}
	{#if time_format === 'time'}{new Date(call.arrival)
			.toLocaleTimeString()
			.replace(/AM|PM/, '')}{:else}{((call.arrival - current_time.ms) / 60_000).toFixed(0)}m{/if}
{/snippet}

<style>
	.trip-content {
		display: flex;
		flex-direction: column;
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}
	.trip-content :global(.trip-stops) {
		flex: 1 1 auto;
		min-height: var(--trip-stops-min-height);
	}
	.preview-panel {
		display: flex;
		flex-direction: column;
		flex: 0 1 auto;
		min-height: 0;
		overflow: auto;
		background: var(--bg-surface-1);
		border-bottom: 1px solid var(--border-default);
	}
	.preview-title {
		flex: none;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		min-height: 28px;
		padding: 0 8px;
		color: var(--text-secondary);
		font-size: 12px;
	}
	.preview-actions {
		display: flex;
		gap: 10px;
		align-items: center;
		white-space: nowrap;
	}
	.preview-actions button {
		width: 28px;
		height: 28px;
		display: grid;
		place-items: center;
		cursor: pointer;
	}
	.stop-row {
		display: flex;
	}
	.passed {
		color: var(--text-muted);
	}
	.select-stop {
		flex: 0 0 44px;
		display: grid;
		place-items: stretch;
		min-height: 44px;
		cursor: pointer;
	}
	.track {
		position: relative;
		align-self: stretch;
		display: grid;
		place-items: center;
	}
	.track::before {
		content: '';
		position: absolute;
		inset-block: 0;
		width: 4px;
		background: var(--text-disabled);
	}
	.selected .track::before {
		background: var(--accent);
	}
	.selected .dot {
		border-color: var(--accent);
		background: var(--accent);
	}
	.select-stop:hover .track::before {
		background: var(--accent-hover);
	}
	.select-stop:hover .dot {
		border-color: var(--accent-hover);
	}
	.open-stop {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		flex: 1;
		min-width: 0;
		min-height: 44px;
		padding-right: 8px;
		text-align: left;
		font-size: 14px;
		cursor: pointer;
	}
	.time {
		flex: none;
		white-space: nowrap;
		font-variant-numeric: tabular-nums;
	}
	.open-stop:hover,
	.select-stop:hover {
		background: var(--bg-hover);
	}
	.stop-row.selected .open-stop,
	.stop-row.selected .select-stop {
		background: var(--accent-bg);
	}
	button:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
</style>
