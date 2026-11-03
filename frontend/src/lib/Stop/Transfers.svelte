<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop } from '#lib/client/index.js';
	import Option from '#lib/Transfers/Option.svelte';
	import type { DepartureOption } from '#lib/transit/nearby.js';
	import { createTransfers } from '#lib/transit/transfers.svelte.js';
	import TransferThread from '#lib/Trip/TransferThread.svelte';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	let {
		stop,
		time_format = 'countdown'
	}: { stop: CatalogStop; time_format?: 'time' | 'countdown' } = $props();
	let open = $state(true);
	let expanded = $state(false);
	const current_time = getCurrentTime();
	const transfers_resource = createTransfers(() => ({
		stop,
		sources: page.data.selected_sources,
		at: current_time.value,
		now: current_time.ms,
		open
	}));
	const transfers = $derived(transfers_resource.current);
	const options = $derived(transfers.options);
	const departures = $derived(transfers.state === 'ready' ? transfers.value : []);
</script>

{#if options.length || transfers.state === 'loading' || transfers.state === 'failed'}
	<section class="transfers" aria-label="Nearby routes">
		<TransferThread label="Nearby routes" bind:open>
			<div role="region" aria-label="Nearby route options">
				{#each departures.slice(0, 3) as departure (departure.key)}
					{@render row(departure)}
				{:else}
					{#if transfers.state === 'loading'}<p class="status" role="status">
							Loading predictions…
						</p>{:else if transfers.state === 'ready'}<p class="status" role="status">
							No nearby predictions to show
						</p>{/if}
				{/each}
				{#if expanded || departures.length > 3}
					<TransferThread label={expanded ? 'Show less' : 'Show more'} nested bind:open={expanded}>
						{#if expanded}
							{#each departures.slice(3) as departure (departure.key)}
								{@render row(departure)}
							{/each}
						{/if}
					</TransferThread>
				{/if}
			</div>
		</TransferThread>
	</section>
{/if}

{#snippet row(departure: DepartureOption)}
	<div class="transfer-row pl-[22px]">
		<Option
			{departure}
			{time_format}
			target="stop"
			show_stop={departure.boarding.stop.name !== stop.name}
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
		max-height: min(220px, 25dvh);
		overflow: auto;
		overscroll-behavior-y: contain;
		scrollbar-width: thin;
		border-bottom: 1px solid var(--border-default);
		padding-block: 4px;
	}
	.transfer-row :global(.option) {
		min-height: 44px;
	}
	.status {
		padding: 6px 8px 6px 22px;
		font-size: 11px;
		color: var(--text-muted);
	}
</style>
