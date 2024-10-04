<script lang="ts">
	import { page } from '$app/stores';
	import { type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import { persisted_rune } from '$lib/util.svelte';
	import SearchWorker from './search_worker?worker';

	let search_worker: Worker;
	let search = $state<'loading' | 'ready'>('loading');

	let bus_stops = $state<Stop<'bus'>[]>([]);
	let train_stops = $state<Stop<'train'>[]>([]);

	$effect(() => {
		console.log('effect');
		const { all_bus_stops, all_train_stops } = $page.data.stops.reduce(
			(acc, stop) => {
				if (stop.type === 'bus') {
					acc.all_bus_stops.push(stop);
				} else if (stop.type === 'train') {
					acc.all_train_stops.push(stop);
				}
				return acc;
			},
			{ all_bus_stops: [], all_train_stops: [] }
		);
		bus_stops = all_bus_stops;
		train_stops = all_train_stops;

		search_worker = new SearchWorker();

		// listen for messages
		search_worker.addEventListener('message', (e) => {
			const { type, payload } = e.data;

			if (type === 'ready') search = 'ready';

			if (type === 'results' && payload.results.length) {
				if (payload.search_type === 'train') train_stops = payload.results;
				else if (payload.search_type === 'bus') bus_stops = payload.results;

				// if (payload.results && payload.results.length < 6) {
				// 	list_el.scrollIntoView();
				// }
			}
		});
		// initialize when the component mounts
		// search_worker.postMessage({
		// 	type: 'load',
		// 	payload: { bus_stops: [...bus_stops], train_stops: [...train_stops] }
		// });
	});

	const stop_pin_rune = persisted_rune<number[]>('stop_pins', []);
</script>

<!-- TODO: Fix large here -->
<!-- {#snippet stop_button(stop: Stop<'bus' | 'train'>)}
	<StopButton {stop} pin_rune={stop_pin_rune} />
{/snippet}

<List
	title="Stops"
	button={stop_button}
	bus_data={bus_stops.slice(0, 20)}
	train_data={train_stops.slice(0, 20)}
	class="mb-16"
/> -->
