<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import type { BusStopTime } from '$lib/bus_api';
	import { bus_stops } from '$lib/stores';
	import TriggerButton from '$lib/components/TriggerButton.svelte';

	export let stop_time: BusStopTime;

	$: stop = $bus_stops.find((s) => s.id === stop_time.stop_id)!;
</script>

<TriggerButton
	state={{
		dialog_id: stop_time.stop_id,
		dialog_type: 'bus_stop',
		dialog_open: true
	}}
>
	<div class="text-left">
		{stop.name}
	</div>
	<div class="text-right">
		{stop_time.arrival.toLocaleTimeString()}
	</div>
</TriggerButton>
