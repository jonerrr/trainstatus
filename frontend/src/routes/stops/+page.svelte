<script lang="ts">
	import { writable } from 'svelte/store';
	import { stops, bus_stops } from '$lib/stores';
	import List from '$lib/components/Stop/List.svelte';

	$: stop_ids = writable<string[]>($stops.slice(0, 15).map((s) => s.id));
	$: bus_stop_ids = writable<number[]>($bus_stops.slice(0, 15).map((s) => s.id));
</script>

<svelte:head>
	<title>Trainstat.us | Stop Times</title>
</svelte:head>

<div class="p-1">
	<List {stop_ids} {bus_stop_ids} title="Stops" show_search={true} />
</div>
