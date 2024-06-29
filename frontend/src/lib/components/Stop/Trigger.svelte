<script lang="ts">
	import { pinned_stops, stop_times } from '$lib/stores';
	import { Direction, StopType, type Stop } from '$lib/api';
	import Pin from '$lib/components/Pin.svelte';
	import Arrivals from '$lib/components/Stop/Arrivals.svelte';
	import TriggerButton from '$lib/components/TriggerButton.svelte';

	export let stop: Stop;

	// Get all possible routes that stop at this stop
	// Base routes are included so we can show them even if they don't have any upcoming trips
	const base_routes = stop.routes
		.filter((r) => r.stop_type === StopType.FullTime || r.stop_type === StopType.PartTime)
		.map((r) => r.id);
	const other_routes = $stop_times
		.filter((st) => st.arrival > new Date() && st.stop_id === stop.id)
		.map((st) => st.route_id);

	$: other_route_ids = Array.from(new Set(other_routes.filter((r) => !base_routes.includes(r))));
</script>

<TriggerButton
	state={{
		dialog_id: stop.id,
		dialog_type: 'stop',
		dialog_open: true
	}}
>
	<div class="w-[25%] grow-0 font-semibold text-indigo-200">
		{stop.name}
	</div>

	<div class="flex flex-col w-[30%] mt-auto">
		<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
			{stop.north_headsign}
		</div>
		<div class="flex flex-col gap-1">
			{#each base_routes as route_id}
				<Arrivals {route_id} {stop} direction={Direction.North} />
			{/each}
			{#each other_route_ids as route_id}
				<Arrivals {route_id} {stop} direction={Direction.North} base_route={false} />
			{/each}
		</div>
	</div>

	<div class="flex flex-col w-[30%] mt-auto">
		<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
			{stop.south_headsign}
		</div>
		<div class="flex flex-col gap-1">
			{#each base_routes as route_id}
				<Arrivals {route_id} {stop} direction={Direction.South} />
			{/each}
			{#each other_route_ids as route_id}
				<Arrivals {route_id} {stop} direction={Direction.South} base_route={false} />
			{/each}
		</div>
	</div>

	<div>
		<Pin item_id={stop.id} store={pinned_stops} />
	</div>
</TriggerButton>
