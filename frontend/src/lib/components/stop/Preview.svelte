<script lang="ts">
	import { createToggle, melt, createSync } from '@melt-ui/svelte';
	import { Drawer } from 'vaul-svelte';
	import { Direction, type Stop } from '$lib/api';
	import { pinned_stops } from '$lib/stores';
	import Pin from '$lib/components/Pin.svelte';
	import Eta from '$lib/components/stop/Eta.svelte';
	import Trips from '$lib/components/stop/Trips.svelte';

	export let stop: Stop;
	const stop_with_eta = stop.trips.map((trip) => {
		const stop_time = trip.stop_times.find((time) => time.stop_id === stop.id)!;

		const arrival = new Date(stop_time.arrival).getTime();
		const now = new Date().getTime();
		const eta = ((arrival - now) / 1000 / 60).toFixed(0);

		// console.log(eta);

		return {
			...trip,
			eta
		};
	});

	console.log(stop.trips);

	const northbound = stop_with_eta.filter((trip) => trip.direction === Direction.North);
	const southbound = stop_with_eta.filter((trip) => trip.direction === Direction.South);

	// const stop_routes = stop.routes.flatMap((route) => route.route_id);
</script>

<Drawer.Root>
	<Drawer.Trigger class="w-full flex justify-between items-center">
		<div class="w-24 grow-0">
			{stop.name}
		</div>

		<!-- northbound trips -->
		<div class="flex grow-0 w-24">
			<Eta routes={stop.routes} stop_id={stop.id} trips={northbound} />
		</div>

		<!-- southbound trips -->
		<div class="flex grow-0 w-24">
			<Eta routes={stop.routes} stop_id={stop.id} trips={southbound} />
		</div>

		<div>
			<Pin item_id={stop.id} store={pinned_stops} />
		</div>
	</Drawer.Trigger>
	<Drawer.Portal>
		<Drawer.Overlay class="fixed inset-0 bg-black/40" />

		<Drawer.Content
			class="bg-neutral-800 max-h-[96%] fixed bottom-0 left-0 right-0 flex flex-col mt-24"
		>
			<div class="mx-auto w-12 h-1.5 flex-shrink-0 rounded-full bg-gray-300 my-2" />

			<Trips />
		</Drawer.Content>
		<Drawer.Overlay />
	</Drawer.Portal>
</Drawer.Root>
<!-- <div class="w-full flex justify-between items-center">
	<div class="w-24 grow-0">
		{stop.name}
	</div>

	<div class="flex grow-0 w-24">
		<Eta routes={stop.routes} stop_id={stop.id} trips={northbound} />
	</div>

	<div class="flex grow-0 w-24">
		<Eta routes={stop.routes} stop_id={stop.id} trips={southbound} />
	</div>

	<div>
		<Pin item_id={stop.id} store={pinned_stops} />
	</div>
</div> -->
