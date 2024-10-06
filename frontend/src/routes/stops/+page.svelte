<script lang="ts">
	import { page } from '$app/stores';
	import { type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import { persisted_rune, stop_pins_rune } from '$lib/util.svelte';
	import SearchWorker from './search_worker?worker';
	import { CircleX } from 'lucide-svelte';

	let search_worker: Worker;
	let search = $state<'loading' | 'ready'>('loading');

	let bus_stops = $state<Stop<'bus'>[]>($page.data.bus_stops.slice(0, 20));
	let train_stops = $state<Stop<'train'>[]>($page.data.train_stops.slice(0, 20));

	$effect(() => {
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

	let search_el: HTMLInputElement;
	let search_term: string = $state('');

	function clearSearch() {
		// reset stop ids
		// $stop_ids = $stop_store.slice(0, 15).map((s) => s.id);
		// $bus_stop_ids = $bus_stop_store.slice(0, 15).map((s) => s.id);
		// search_el.value = '';
		search_term = '';
	}

	function searchStops(e: { target: { value: string } }) {
		// If search is empty, clear search and show all stops
		if (e.target.value === '') {
			clearSearch();
			return;
		}

		search_term = e.target.value;
		// searchWorker.postMessage({
		// 	type: 'search',
		// 	payload: { search_term, search_type: $tab_value }
		// });
	}

	// from https://www.okupter.com/blog/svelte-debounce
	function debounce(callback: Function, wait = 200) {
		let timeout: ReturnType<typeof setTimeout>;

		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => callback(...args), wait);
		};
	}
</script>

<!-- TODO: Fix large here -->
{#snippet stop_button(stop: Stop<'bus' | 'train'>)}
	<StopButton {stop} pin_rune={stop_pins_rune} large />
{/snippet}

{#snippet search_bar()}
	<div class="relative">
		<input
			bind:this={search_el}
			bind:value={search_term}
			oninput={debounce(search_term)}
			type="search"
			placeholder={search === 'ready' ? 'Search stops' : 'Loading search...'}
			class="search-stops text-indigo-200 max-w-[calc(100dvw)] pl-10 z-20 w-full h-12 rounded bg-neutral-900 shadow-2xl border-neutral-800/20 ring-1 ring-inset ring-neutral-700 placeholder:text-neutral-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600"
		/>
		<button
			aria-label="Clear search"
			class="z-30 w-6 h-6 text-indigo-600 hover:text-indigo-700 active:text-indigo-700 absolute right-2 my-auto top-1/2 transform -translate-y-1/2"
			onclick={clearSearch}
		>
			<CircleX />
		</button>
	</div>
{/snippet}

<List
	title="Stops"
	button={stop_button}
	bus_data={bus_stops}
	train_data={train_stops}
	class="mb-16"
/>

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
