<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop } from '#lib/client/index.js';
	import type { DepartureOption } from '#lib/data/nearby.js';
	import { createTransfers } from '#lib/data/transfers.svelte.js';
	import Option from '#lib/transfers/Option.svelte';
	import TransferThread from '#lib/trip/TransferThread.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';

	let { stop, timeFormat = 'countdown' }: { stop: CatalogStop; timeFormat?: 'time' | 'countdown' } =
		$props();
	let open = $state(true);
	let expanded = $state(false);
	const currentTime = getCurrentTime();
	const transfersResource = createTransfers(() => ({
		stop,
		sources: page.data.selectedSources,
		at: currentTime.value,
		now: currentTime.ms,
		open
	}));
	const transfers = $derived(transfersResource.current);
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
			{timeFormat}
			target="stop"
			showStop={departure.boarding.stop.name !== stop.name}
			showSource={options.some(
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
