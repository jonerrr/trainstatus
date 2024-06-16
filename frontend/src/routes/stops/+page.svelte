<script lang="ts">
	import List from '$lib/components/Stop/List.svelte';
</script>

<svelte:head>
	<title>Trainstat.us | Stop Times</title>
</svelte:head>

<div class="p-1">
	<List bus_stop_ids={null} stop_ids={null} title="Stops" show_search={true} />
</div>
