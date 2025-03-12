<script lang="ts">
	import { LayerCake } from 'layercake';
	import { page } from '$app/state';
	import { trips } from '$lib/trips.svelte';
	import { stop_times } from '$lib/stop_times.svelte';
	import type { Route } from '$lib/static';

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
	let direction = $state('northbound');
	$inspect(direction);
</script>

<svelte:head>
	<title>Charts</title>
</svelte:head>

<div class="flex flex-col gap-1 text-white p-1">
	<div class="text-xl">Select Options</div>
	<div class="grid grid-rows-2 gap-2">
		<div class="flex items-center">
			<input
				bind:group={direction}
				type="radio"
				id="northbound"
				name="direction"
				value="northbound"
				class="mr-2"
			/>
			<label for="northbound">northbound</label>
		</div>
		<div class="flex items-center">
			<input
				bind:group={direction}
				type="radio"
				id="southbound"
				name="direction"
				value="southbound"
				class="mr-2"
			/>
			<label for="southbound">southbound</label>
		</div>
	</div>
</div>

<div class="w-full h-full"></div>
