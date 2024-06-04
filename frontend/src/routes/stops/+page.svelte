<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import List from '$lib/components/Stop/List.svelte';
</script>

<svelte:head>
	<title>Trainstat.us | Stop Times</title>
</svelte:head>

<!-- TODO: combine alert and stop list into one component and reuse across pages -->
<!-- TODO: show routes even when there is no alert for them -->

<div class="">
	<List stop_ids={null} title="Stops" show_search={true} />
</div>
