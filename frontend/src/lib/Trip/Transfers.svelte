<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import type { Call } from '#lib/transit/decode.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	interface Props {
		transfer_calls: (Call & { source: Source })[];
		time_format: 'time' | 'countdown';
	}

	const { time_format, transfer_calls }: Props = $props();
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center justify-evenly gap-2 overflow-x-auto">
		{#each transfer_calls as call (`${call.source}:${call.trip_id}:${call.sequence}`)}
			{const route = $derived(transit.peekCatalog(call.source)?.routes_by_id.get(call.route_id))}
			{#if route}
				<button
					onclick={() =>
						open_modal({
							type: 'trip',
							source: call.source,
							route_id: call.route_id,
							id: call.trip_id
						})}
					class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
				>
					<Icon width={18} height={18} {route} link={false} />
					{#if time_format === 'time'}
						{new Date(call.arrival).toLocaleTimeString().replace(/AM|PM/, '')}
					{:else}
						{((call.arrival - current_time.ms) / 1000 / 60).toFixed(0)}m
					{/if}
				</button>
			{/if}
		{/each}
	</div>
</div>
