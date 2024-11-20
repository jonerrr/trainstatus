<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import { persisted_rune, stop_pins_rune } from '$lib/util.svelte';
	import SearchWorker from './search_worker?worker';
	// import { untrack } from 'svelte';

	let search_worker: Worker;
	let search = $state<'loading' | 'ready'>('loading');

	let bus_stops = $state<Stop<'bus'>[]>($page.data.bus_stops.slice(0, 15));
	let train_stops = $state<Stop<'train'>[]>($page.data.train_stops.slice(0, 15));

	let selected_tab = $state(persisted_rune<'train' | 'bus'>('stops_tab', 'train'));

	$effect(() => {
		if (!search_worker) {
			// console.log('init search worker');
			search_worker = new SearchWorker();
		}

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
		search_worker.postMessage({
			type: 'load',
			payload: {
				bus_stops: JSON.parse(JSON.stringify($page.data.bus_stops)),
				train_stops: JSON.parse(JSON.stringify($page.data.train_stops))
			}
		});
	});

	// let search_el: HTMLInputElement;
	let search_input: string = $state('');
	// let search_term = $derived.by(debounce(() => search_input, 300));
	// $inspect(search_term);
	function clear_search() {
		// reset stop ids
		bus_stops = $page.data.bus_stops.slice(0, 15);
		train_stops = $page.data.train_stops.slice(0, 15);

		search_input = '';
	}

	// let debounce_timeout = $state<number>();
	// let debounce_timeout: number;

	// $effect(() => {
	// 	if (search !== 'ready') return;

	// 	if (search_input === '') {
	// 		clear_search();
	// 		return;
	// 	}

	// 	clearTimeout(debounce_timeout);

	// 	debounce_timeout = setTimeout(() => {
	// 		console.log('updating stops');
	// 		// if (search_input === '') {
	// 		// 	clear_search();
	// 		// } else {
	// 		search_worker.postMessage({
	// 			type: 'search',
	// 			payload: { search_term: search_input, search_type: selected_tab.value }
	// 		});
	// 		// }
	// 	}, 150);
	// });
	// if (search === 'ready') {
	// 	// console.log('searching stops');
	// 	search_worker.postMessage({
	// 		type: 'search',
	// 		payload: { search_term, search_type: selected_tab.value }
	// 	});
	// }
	// });

	$effect(() => {
		if (search !== 'ready') return;

		if (search_input === '') {
			clear_search();
		} else {
			search_worker.postMessage({
				type: 'search',
				payload: { search_term: search_input, search_type: selected_tab.value }
			});
		}

		// if (search === 'ready') {
		// 	// console.log('searching stops');
		// 	search_worker.postMessage({
		// 		type: 'search',
		// 		payload: { search_term, search_type: selected_tab.value }
		// 	});
		// }
	});
</script>

<svelte:head>
	<title>Stops</title>
</svelte:head>

<!-- TODO: Fix large here -->
{#snippet stop_button(stop: Stop<'bus' | 'train'>)}
	<StopButton {stop} pin_rune={stop_pins_rune} />
{/snippet}

<List
	title="Stops"
	button={stop_button}
	bus_data={bus_stops}
	train_data={train_stops}
	monitor_routes
	class="max-h-[calc(100dvh-13.1rem)]"
	auto_scroll
	bind:selected_tab
/>

<div class="absolute bottom-1 w-full">
	<input
		name="search"
		bind:value={search_input}
		type="search"
		placeholder={search === 'ready' ? 'Search stops' : 'Loading search...'}
		class="search-stops w-full h-12 text-neutral-200 pl-10 rounded bg-neutral-900 border-neutral-800 ring-1 ring-inset ring-neutral-600 focus:ring-neutral-400 focus:border-neutral-400 focus:ring-2 focus:ring-inset placeholder:text-neutral-400"
	/>
	<button
		aria-label="Clear search"
		class="z-30 w-6 h-6 text-neutral-200 hover:text-neutral-400 active:text-neutral-400 absolute right-2 my-auto top-1/2 transform -translate-y-1/2"
		onclick={clear_search}
	>
		<CircleX />
	</button>
</div>

<style lang="postcss">
	.search-stops {
		background-image: url('/search.svg');

		background-position: 10px 10px;
		background-repeat: no-repeat;
	}

	/* Remove default styles from search */
	input[type='search']::-webkit-search-decoration,
	input[type='search']::-webkit-search-cancel-button,
	input[type='search']::-webkit-search-results-button,
	input[type='search']::-webkit-search-results-decoration {
		-webkit-appearance: none;
	}
</style>
