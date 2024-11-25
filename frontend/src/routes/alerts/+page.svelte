<script lang="ts">
	import { page } from '$app/stores';
	import { route_pins_rune } from '$lib/util.svelte';
	import List from '$lib/List.svelte';
	import type { Route } from '$lib/static';

	const { bus_routes, train_routes } = $derived(
		Object.values($page.data.routes).reduce(
			(acc: { bus_routes: Route[]; train_routes: Route[] }, route) => {
				if (route.route_type === 'bus') {
					acc.bus_routes.push(route);
				} else {
					acc.train_routes.push(route);
				}
				return acc;
			},
			{ bus_routes: [], train_routes: [] }
		)
	);
</script>

<svelte:head>
	<title>Alerts</title>
</svelte:head>

<List
	title="Route Alerts"
	bus_data={bus_routes}
	train_data={train_routes}
	type="route"
	pin_rune={route_pins_rune}
	class="max-h-[calc(100dvh-10.5rem)]"
	height_calc={() => 45}
	ssr_min={20}
/>
