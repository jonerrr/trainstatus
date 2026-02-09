<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import type { Route } from '$lib/static';
	import { route_pins } from '$lib/stores.svelte';

	// const { bus_routes, train_routes } = $derived(
	// 	Object.values(page.data.routes).reduce(
	// 		(acc: { bus_routes: Route[]; train_routes: Route[] }, route) => {
	// 			if (route.route_type === 'bus') {
	// 				acc.bus_routes.push(route);
	// 			} else {
	// 				acc.train_routes.push(route);
	// 			}
	// 			return acc;
	// 		},
	// 		{ bus_routes: [], train_routes: [] }
	// 	)
	// );
</script>

<!-- <svelte:head>
	<title>Alerts | Train Status</title>
</svelte:head> -->

<List
	title="Route Alerts"
	sources={page.data.routes}
	type="route"
	pins={route_pins}
	class="max-h-[calc(100dvh-10.5rem)]"
	height_calc={() => 45}
	ssr_min={20}
/>
