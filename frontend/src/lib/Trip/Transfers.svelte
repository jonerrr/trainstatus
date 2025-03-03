<script lang="ts">
	import { page } from '$app/state';
	import { type StopTime } from '$lib/stop_times.svelte';
	import { is_train_route } from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import { pushState } from '$app/navigation';

	interface TransferProps {
		transfer_stop_times: StopTime[];
		time_format: 'time' | 'countdown';
	}

	const { time_format, transfer_stop_times }: TransferProps = $props();
</script>

<div class="flex flex-col px-1 bg-neutral-900 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex gap-2 items-center justify-evenly overflow-x-auto">
		{#each transfer_stop_times as st (st.trip_id)}
			{@const route = page.data.routes[st.trip.route_id]}
			<button
				onclick={() => pushState('', { modal: 'trip', data: st.trip })}
				class="transition-colors duration-200 flex rounded-sm bg-neutral-800 shadow-2xl gap-1 p-1 items-center hover:bg-neutral-700 active:bg-neutral-900"
			>
				<Icon
					{route}
					link={false}
					express={is_train_route(route, st.trip) && st.trip.data.express}
				/>
				{#if time_format === 'time'}
					{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
				{:else}
					{((st.arrival.getTime() - new Date().getTime()) / 1000 / 60).toFixed(0)}m
				{/if}
			</button>
		{/each}
	</div>
</div>
