<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop, ServiceTrip } from '#lib/client/index.js';
	import Option from '#lib/Transfers/Option.svelte';
	import {
		compareTransfers,
		type DepartureOption,
		type IncomingTrip
	} from '#lib/transit/nearby.js';
	import { createTransfers } from '#lib/transit/transfers.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import { arrivalExits, arrivalTransferExits } from './platform-exits.js';
	import PlatformExits from './PlatformExits.svelte';
	import TransferThread from './TransferThread.svelte';

	let {
		stop,
		incoming,
		time_format,
		trip,
		platform_edges = [],
		open = $bindable(true)
	}: {
		stop: CatalogStop;
		incoming: IncomingTrip;
		time_format: 'time' | 'countdown';
		trip?: ServiceTrip;
		platform_edges?: readonly string[];
		open?: boolean;
	} = $props();
	const current_time = getCurrentTime();
	let expanded = $state(false);
	let later_open = $state(false);
	let selected_key = $state<string | null>(null);
	const transfers_resource = createTransfers(() => ({
		stop,
		incoming,
		sources: page.data.selected_sources,
		at: current_time.value,
		now: current_time.ms,
		open
	}));
	const transfers = $derived(transfers_resource.current);
	const options = $derived(transfers.options);
	const ordered = $derived(transfers.state === 'ready' ? transfers.value : []);
	const later = $derived(ordered.flatMap((o) => (o.next ? [o.next] : [])).sort(compareTransfers));
	const has_details = $derived(expanded || ordered.length > 3 || ordered.some((o) => o.next));
	const selection = $derived.by(() => {
		if (!trip) return undefined;
		const departure = ordered
			.flatMap((o) => (o.next ? [o, o.next] : [o]))
			.find((o) => key(o) === selected_key);
		const exits = departure && arrivalTransferExits(stop, trip, platform_edges, departure);
		return departure && exits ? { departure, exits } : undefined;
	});
	const selected = $derived(selection?.departure);
	const guidance = $derived(
		trip ? (selection?.exits ?? arrivalExits(stop, trip, platform_edges)) : null
	);
	function key(departure: DepartureOption) {
		return JSON.stringify([departure.key, departure.call.trip_id, departure.call.sequence]);
	}
</script>

{#if trip?.data.source === 'mta_subway'}
	<PlatformExits {guidance} {trip} transfer={selected} />
{/if}
{#if transfers.state !== 'failed' && (options.length || transfers.state === 'loading')}
	<section class="transfers" aria-label="Transfers after arrival">
		<TransferThread label="Transfers" bind:open>
			{#each ordered.slice(0, 3) as departure (key(departure))}{@render row(departure)}
			{:else}{#if transfers.state === 'loading'}<p class="status" role="status">
						Loading predictions…
					</p>{:else if transfers.state === 'ready'}<p class="status" role="status">
						No transfer predictions to show
					</p>{/if}{/each}
			{#if has_details}
				<TransferThread label={expanded ? 'Show less' : 'Show more'} nested bind:open={expanded}>
					{#if expanded}
						{#each ordered.slice(3) as departure (key(departure))}{@render row(departure)}{/each}
						{#if later.length}
							<TransferThread label="Later departures" nested bind:open={later_open}>
								{#if later_open}
									<div role="group" aria-label="Later departures">
										{#each later as departure (key(departure))}{@render row(departure)}{/each}
									</div>
								{/if}
							</TransferThread>
						{/if}
					{/if}
				</TransferThread>
			{/if}
		</TransferThread>
	</section>
{/if}

{#snippet row(departure: DepartureOption)}
	{const transfer_exits = $derived(
		trip ? arrivalTransferExits(stop, trip, platform_edges, departure) : null
	)}
	{const can_preview = $derived(!!transfer_exits)}
	{const exit_cars = $derived(
		transfer_exits ? [...new Set(transfer_exits.exits.map((exit) => exit.car))] : []
	)}
	{const active = $derived(selected !== undefined && key(departure) === key(selected))}
	<div class={['transfer-row relative pl-[22px]', { 'active bg-accent-bg': active }]}>
		{#if can_preview}<button
				type="button"
				class={[
					'preview absolute inset-y-0 -left-[22px] z-2 grid w-11 cursor-pointer place-items-center',
					{ active }
				]}
				aria-label={`Preview ${departure.route.short_name} to ${departure.destination}`}
				aria-pressed={active}
				title="Preview platform guidance"
				onclick={() => (selected_key = active ? null : key(departure))}
				><span
					class="car-box relative z-1 inline-flex h-[17px] max-w-[38px] min-w-5 items-center justify-center rounded-none border-[1.5px] border-muted bg-surface-1 px-[3px] text-[10px] leading-none font-bold tracking-[-0.03em] whitespace-nowrap text-secondary tabular-nums"
					>{exit_cars.slice(0, 3).join('·')}</span
				></button
			>{/if}
		<Option
			{departure}
			{time_format}
			arrival={incoming.arrival}
			show_stop={!departure.boarding.same_stop}
			show_source={options.some(
				(o) =>
					o.route.short_name === departure.route.short_name &&
					o.route.data.source !== departure.route.data.source
			)}
		/>
	</div>
{/snippet}

<style>
	.transfers {
		flex: 0 1 auto;
		min-height: 37px;
		max-height: min(220px, 28dvh);
		overflow: auto;
		overscroll-behavior-y: contain;
		scrollbar-width: thin;
		border-top: 1px solid var(--border-strong);
		padding-block: 4px 0;
	}
	.preview::before {
		content: '';
		position: absolute;
		left: 21px;
		inset-block: 0;
		width: 2px;
		background: transparent;
	}
	.preview.active::before {
		left: 20px;
		width: 4px;
		background: var(--accent);
	}
	.preview:hover::before {
		background: var(--accent-hover);
	}
	.preview:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
	.preview:hover .car-box {
		border-color: var(--accent-hover);
		color: #ffffff;
	}
	.preview.active .car-box {
		border-color: var(--accent);
		background: var(--accent);
		color: var(--accent-fg);
	}
	.status {
		padding: 6px 8px 6px 22px;
		font-size: 11px;
		color: var(--text-muted);
	}
</style>
