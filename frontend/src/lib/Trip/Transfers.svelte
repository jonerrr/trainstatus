<script lang="ts">
	import { pushState } from '$app/navigation';
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';

	import type { StopTime } from '@trainstatus/client';

	interface Props {
		transfer_stop_times: StopTime[];
		time_format: 'time' | 'countdown';
	}

	const { time_format, transfer_stop_times }: Props = $props();

	const trips = trip_context.get();
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center justify-evenly gap-2 overflow-x-auto">
		{#each transfer_stop_times as st (st.trip_id)}
			{@const trip = trips[st.data.source].value?.get(st.trip_id)!}
			{@const route = page.data.routes_by_id[st.data.source][trip.route_id]}
			<button
				onclick={() => pushState('', { modal: { type: 'trip', ...trip } })}
				class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
			>
				<Icon width={18} height={18} {route} link={false} />
				{#if time_format === 'time'}
					{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
				{:else}
					{((st.arrival.getTime() - new Date().getTime()) / 1000 / 60).toFixed(0)}m
				{/if}
			</button>
		{/each}
	</div>
</div>
