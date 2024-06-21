<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import { TrainStatus, type StopTime } from '$lib/api';
	import { stops } from '$lib/stores';

	export let stop_time: StopTime;
	export let stop_id: string | null;
	export let train_status: TrainStatus | null;

	$: stop = $stops.find((s) => s.id === stop_time.stop_id)!;
</script>

<button
	class="border-neutral-600 w-full bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1 flex justify-between"
	transition:slide={{ easing: quintOut, axis: 'y' }}
	on:click={() => {
		pushState('', {
			dialog_id: stop_time.stop_id,
			dialog_type: 'stop',
			dialog_open: true
		});
	}}
>
	<div class="text-left">
		{stop.name}
		{#if stop_time.stop_id === stop_id}
			<span class="text-indigo-200 text-xs">
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
	<div class="text-right">
		{stop_time.arrival.toLocaleTimeString()}
	</div>
</button>
