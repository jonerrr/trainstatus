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

<List
	{stop_ids}
	{bus_stop_ids}
	title="Stops"
	manage_height={false}
	class="max-h-[calc(100dvh-7.5rem)] overflow-auto"
	show_search
/>
