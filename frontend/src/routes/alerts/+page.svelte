<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import List from '$lib/components/RouteAlert/List.svelte';
</script>

<svelte:head>
	<title>Trainstat.us | Alerts</title>
</svelte:head>

<!-- TODO: combine alert and stop list into one component and reuse across pages -->
<!-- TODO: show routes even when there is no alert for them -->

<div class="p-1">
	<List manage_height={false} />
</div>
