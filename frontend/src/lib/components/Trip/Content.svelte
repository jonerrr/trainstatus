<script lang="ts">
	import { Share, ClipboardCheck } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import type { Trip } from '$lib/api';
	import { stops, trips, stop_times } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Times from '$lib/components/Trip/Times.svelte';

	export let trip_id: string;

	const trip: Trip = $trips.find((t) => t.id === trip_id)!;

	$: trip_stop_times = $stop_times.filter((st) => st.trip_id === trip_id);

	$: last_stop_name = $stops.find(
		(s) => s.id === trip_stop_times[trip_stop_times.length - 1].stop_id
	)!.name;

	let copied = false;

	function share() {
		let url = window.location.origin + `/?t=${trip_id}`;
		if (!navigator.share) {
			navigator.clipboard.writeText(url);
			// set copied to true and change back in 500 ms
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 800);
		} else {
			navigator.share({
				// TODO: maybe include next stop and route name
				// TODO: custom embeds
				title: `View my subway trip`,
				url
			});
		}
	}

	// TODO: add button to load previous stop times and fetch from api
</script>

<!-- list of stops and their arrival times -->
<div
	class="relative overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[80vh]
	max-w-[450px]"
>
	<div class="flex items-center justify-between bg-neutral-800 w-full">
		<div class="flex gap-2">
			<Icon width="2rem" height="2rem" name={trip.route_id} />

			<h2 class="font-bold text-xl text-indigo-300">
				{last_stop_name}
			</h2>
		</div>

		<div class="pr-2">
			{#if !copied}
				<button aria-label="Share trip" on:click={share}>
					<Share class="h-6 w-6" />
				</button>
			{:else}
				<button class="text-green-600" aria-label="Trip link copied to clipboard">
					<ClipboardCheck class="h-6 w-6" />
				</button>
			{/if}
		</div>
	</div>

	<!-- TODO: figure out why the train times are not changing the stop name on update -->
	{#if trip_stop_times.length}
		<div class="max-h-full">
			{#each trip_stop_times as stop_time}
				<Times bind:stop_time />
			{/each}
		</div>
	{/if}
</div>
