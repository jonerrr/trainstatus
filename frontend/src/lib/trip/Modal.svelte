<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import type { Call } from '#lib/data/decode.js';
	import { getTransitData, type TripRead } from '#lib/data/store.svelte.js';
	import ModalList from '#lib/ModalList.svelte';
	import ModalListSkeleton from '#lib/ModalListSkeleton.svelte';
	import Icon from '#lib/route/Icon.svelte';
	import Transfers from '#lib/trip/Transfers.svelte';
	import { getCurrentTime, openModal } from '#lib/url-params.svelte.js';
	import { tripDestinationTitle, tripHeadsign } from '#lib/util.svelte.js';
	import VehicleCapacity from '#lib/VehicleCapacity.svelte';

	import { RotateCcw } from '@lucide/svelte';

	import { tripVehicleSummary } from './status.js';

	const currentTime = getCurrentTime();

	interface Props {
		showPrevious: boolean;
		timeFormat: 'time' | 'countdown';
		source: Source;
		routeId: string;
		tripId: string;
	}

	const { source, routeId, tripId, showPrevious, timeFormat }: Props = $props();
	const transit = getTransitData();
	const catalogResource = transit.catalog(() => source);
	const tripResource = transit.trip(() => ({
		source,
		routeId,
		tripId,
		at: currentTime.value
	}));
	const catalogRead = $derived(catalogResource.current);
	const tripRead = $derived(tripResource.current);

	// The header stays mounted across loading states and shows whatever has loaded.
	const loadedCatalog = $derived(catalogRead.state === 'ready' ? catalogRead.value : undefined);
	const route = $derived(loadedCatalog?.routesById.get(routeId));
	const loadedRead = $derived(tripRead.state === 'ready' ? tripRead.value : undefined);
	let selectedKey = $state<string | null>(null);
	let transfersOpen = $state(true);
	let stopHeights = $state<Record<number, number>>({});
	const calls = $derived(loadedRead ? visibleCalls(loadedRead) : []);
	const stopsMinHeight = $derived(
		2 + calls.slice(0, 3).reduce((height, call) => height + (stopHeights[call.sequence] ?? 45), 0)
	);
	const selectedCall = $derived(calls.find((call) => visitKey(call) === selectedKey) ?? calls[0]);
	const selectedStop = $derived(
		selectedCall ? loadedCatalog?.stopsById.get(selectedCall.stopId) : undefined
	);
	function visitKey(call: Call) {
		return JSON.stringify([source, routeId, tripId, call.sequence]);
	}

	function visibleCalls(read: TripRead) {
		const now = currentTime.ms;
		const ordered = [...read.calls].sort((a, b) => a.sequence - b.sequence);
		// Charts only show trips that have already passed.
		if (showPrevious || page.url.pathname.startsWith('/charts')) return ordered;
		return ordered.filter((call) => call.arrival > now);
	}
</script>

{#snippet title_row(read: TripRead | undefined)}
	{const status = $derived(read?.trip.status)}
	{const isBus = $derived(source === 'mta_bus' || source === 'njt_bus')}
	{const vehicle = $derived(read ? tripVehicleSummary(read.trip) : undefined)}
	<div class="shrink-0 border-b border-border-default p-3">
		<div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-x-3">
			{#if route}
				<Icon
					{route}
					width={28}
					height={28}
					link
					showAlerts
					class="col-start-1 row-start-1 self-center justify-self-center"
				/>
			{/if}
			{#if isBus && read}
				<div
					class={[
						'flex h-5 items-center justify-self-center leading-5',
						route ? 'col-start-1 row-start-2' : 'col-start-1 row-start-1'
					]}
				>
					<VehicleCapacity {status} />
				</div>
			{/if}
			{#if vehicle?.vehicleLabel}
				<span
					class="col-start-3 row-start-1 flex min-h-7 shrink-0 items-center self-center text-xs leading-4 text-muted tabular-nums"
					>{vehicle.vehicleLabel}</span
				>
			{/if}
			<div class="col-start-2 row-start-1 flex min-h-7 min-w-0 items-center self-center text-left">
				<div class="text-sm leading-4 font-semibold wrap-anywhere text-primary">
					{#if read}
						{tripDestinationTitle(
							tripHeadsign(read.trip, route, read.calls, loadedCatalog?.stopsById),
							route
						)}
					{:else}
						{route?.long_name ?? ''}
					{/if}
				</div>
			</div>
			{#if vehicle?.deviationText}
				<div
					class={[
						'col-start-2 row-start-2 flex h-5 items-center text-sm leading-5',
						vehicle.deviationTone === 'warning' ? 'text-status-warning' : 'text-secondary'
					]}
				>
					{vehicle.deviationText}
				</div>
			{/if}
		</div>
	</div>
{/snippet}

<div class="trip-content" style:--trip-stops-min-height={`${stopsMinHeight}px`}>
	{@render title_row(loadedRead)}
	<svelte:boundary>
		{const catalog = $derived(catalogRead.state === 'ready' ? catalogRead.value : undefined)}
		{const read = $derived(tripRead.state === 'ready' ? tripRead.value : undefined)}

		{#if catalog && !route}
			<p class="p-2 text-center text-muted">Route not found</p>
		{:else if catalog && read}
			{#if selectedCall && selectedStop}
				<div class="preview-panel" aria-label="Selected stop preview" role="region">
					<div class="preview-title">
						<span>{selectedStop.name}</span><span class="preview-actions"
							>{#if selectedCall !== calls[0]}<button
									type="button"
									onclick={() => (selectedKey = null)}
									aria-label="Reset to next stop"
									title="Reset to next stop"><RotateCcw size={14} /></button
								>{/if}{@render arrival_time(selectedCall)}</span
						>
					</div>
					{#key visitKey(selectedCall)}
						<Transfers
							stop={selectedStop}
							incoming={{
								source,
								routeId,
								direction: read.trip.direction,
								arrival: selectedCall.arrival
							}}
							{timeFormat}
							trip={read.trip}
							platformEdges={selectedCall.platformEdges}
							bind:open={transfersOpen}
						/>
					{/key}
				</div>
			{/if}
			<ModalList class="trip-stops">
				{#each calls as call (call.sequence)}
					{const stop = $derived(catalog?.stopsById.get(call.stopId))}
					{#if stop}
						<div
							bind:offsetHeight={stopHeights[call.sequence]}
							class={[
								'stop-row',
								{ selected: selectedCall === call, passed: call.arrival < currentTime.ms }
							]}
						>
							<button
								type="button"
								class="select-stop"
								aria-label={`Preview ${stop.name}`}
								aria-pressed={selectedCall === call}
								title={`Preview exits and transfers at ${stop.name}`}
								onclick={() => (selectedKey = visitKey(call))}
								><span class="track" aria-hidden="true"
									><span class="dot relative size-3.5 rounded-none border-3 border-muted bg-canvas"
									></span></span
								></button
							>
							<button
								type="button"
								class="open-stop"
								aria-label={`Open ${stop.name} stop modal`}
								onclick={() => openModal({ type: 'stop', source, id: stop.id })}
								><span>{stop.name}</span><span class="time">{@render arrival_time(call)}</span
								></button
							>
						</div>
					{/if}
				{:else}<p class="p-2 text-center text-muted">No predictions to show</p>{/each}
			</ModalList>
		{:else if catalogRead.state === 'loading' || tripRead.state === 'loading'}
			<ModalListSkeleton rows={6} timeline />
		{:else if catalog && tripRead.state === 'missing'}
			<p class="p-2 text-center text-muted">This trip is not currently running.</p>
		{/if}
	</svelte:boundary>
</div>

{#snippet arrival_time(call: Call)}
	{#if timeFormat === 'time'}{new Date(call.arrival)
			.toLocaleTimeString()
			.replace(/AM|PM/, '')}{:else}{((call.arrival - currentTime.ms) / 60_000).toFixed(0)}m{/if}
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
