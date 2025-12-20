<script lang="ts">
	import { pushState } from '$app/navigation';
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import { type StopTime } from '$lib/stop_times.svelte';
	import { is_train_route } from '$lib/trips.svelte';

	interface TransferProps {
		transfer_stop_times: StopTime[];
		time_format: 'time' | 'countdown';
	}

	const { time_format, transfer_stop_times }: TransferProps = $props();
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center justify-evenly gap-2 overflow-x-auto">
		{#each transfer_stop_times as st (st.trip_id)}
			{@const route = page.data.routes[st.trip.route_id]}
			<button
				onclick={() => pushState('', { modal: 'trip', data: st.trip })}
				class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
			>
				<Icon
					width={18}
					height={18}
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
