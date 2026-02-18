<script lang="ts">
	import { pushState } from '$app/navigation';
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import { trip_context } from '$lib/sources/trips.svelte';

	import type { StopTime } from '@trainstatus/client';

	interface TransferProps {
		transfer_stop_times: StopTime[];
		time_format: 'time' | 'countdown';
	}

	const { time_format, transfer_stop_times }: TransferProps = $props();

	const trips = trip_context.get()['mta_subway']; // TODO: don't hardcode mta_subway
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center justify-evenly gap-2 overflow-x-auto">
		{#each transfer_stop_times as st (st.trip_id)}
			{@const trip = trips.value?.get(st.trip_id)!}
			<!-- TODO: don't hardcode mta_subway. but right now its ok because mta_subway is the only one with transfers -->
			{@const route = page.data.routes_by_id['mta_subway'][trip.route_id]}
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
