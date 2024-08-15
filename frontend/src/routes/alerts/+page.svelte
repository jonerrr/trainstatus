<script lang="ts">
	import { all_route_ids } from '$lib/api';
	import { bus_routes } from '$lib/stores';
	import List from '$lib/components/RouteAlert/List.svelte';
</script>

<!-- <svelte:head>
	<title>Trainstat.us | Alerts</title>
</svelte:head> -->

<List
	route_ids={all_route_ids}
	bus_route_ids={$bus_routes.map((r) => r.id)}
	manage_height={false}
	show_search
/>
