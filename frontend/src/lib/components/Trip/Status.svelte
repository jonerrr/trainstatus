<script lang="ts">
	import { TrainStatus, type Stop } from '$lib/api';

	export let train_status: TrainStatus | null;
	export let stop: Stop | undefined;
</script>

{#if train_status === TrainStatus.AtStop}
	<div class="text-neutral-400">Arrived at {stop?.name}</div>
{:else if train_status === TrainStatus.InTransitTo}
	<div class="text-neutral-400">In transit to {stop?.name}</div>
{:else if train_status === TrainStatus.Incoming}
	<div class="text-neutral-400">Arriving at {stop?.name}</div>
	<!-- {:else if !trip.assigned}
<div class="text-neutral-400">Not assigned</div> -->
{:else}
	<div class="text-neutral-400">Next stop: {stop?.name}</div>
{/if}
