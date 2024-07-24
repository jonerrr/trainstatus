<script lang="ts">
	import { TrainStatus, type StopTime } from '$lib/api';
	import { stops } from '$lib/stores';
	import TriggerButton from '$lib/components/TriggerButton.svelte';

	export let stop_time: StopTime;
	export let stop_id: string | null;
	export let train_status: TrainStatus | null;

	$: stop = $stops.find((s) => s.id === stop_time.stop_id)!;
</script>

<TriggerButton
	state={{
		dialog_id: stop_time.stop_id,
		dialog_type: 'stop',
		dialog_open: true
	}}
>
	<div class="text-left">
		{stop.name}
		{#if stop_time.stop_id === stop_id}
			<span class="text-indigo-400 text-xs">
				{#if train_status === TrainStatus.AtStop}
					(at stop)
				{:else if train_status === TrainStatus.InTransitTo}
					(approaching)
				{:else if train_status === TrainStatus.Incoming}
					(arriving)
				{/if}
			</span>
		{/if}
	</div>
	{#if stop_time.arrival > new Date()}
		<div class={`text-right`}>
			{stop_time.arrival.toLocaleTimeString()}
		</div>
	{:else}
		<div class={`text-right text-neutral-400`}>
			{stop_time.arrival.toLocaleTimeString()}
		</div>
	{/if}
</TriggerButton>
