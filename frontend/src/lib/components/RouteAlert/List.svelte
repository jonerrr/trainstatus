<script lang="ts">
	import { persisted } from 'svelte-persisted-store';
	import { CircleX } from 'lucide-svelte';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[];
	export let bus_route_ids: string[];
	export let title: string = 'Alerts';
	export let manage_height: boolean = true;
	export let show_search = false;

	let list_el: List;
	let search_term = '';
	$: shown_bus_route_ids = bus_route_ids;

	let tab_value = persisted(`${title.toLowerCase()}_tab`, 'Train');
</script>

<List
	bind:this={list_el}
	bind:tab_value
	class={$$props.class ?? undefined}
	bind:manage_height
	bind:title
>
	<div slot="train" class="divide-y divide-neutral-800">
		{#each route_ids as route_id (route_id)}
			<Trigger {route_id} route_type="route_alert" />
		{/each}
	</div>

	<div slot="bus" class="divide-y divide-neutral-800">
		{#each shown_bus_route_ids as route_id (route_id)}
			<Trigger {route_id} route_type="bus_route_alert" />
		{/each}
	</div>

	{#if show_search && $tab_value === 'Bus'}
		<div class="relative">
			<input
				bind:value={search_term}
				on:input={(e) => {
					if (search_term === '') {
						shown_bus_route_ids = bus_route_ids;
						search_term = '';
						return;
					}

					shown_bus_route_ids = bus_route_ids.filter((route_id) =>
						route_id.includes(search_term.toUpperCase())
					);
					list_el.scrollIntoView();
				}}
				type="search"
				placeholder="Search bus route"
				class="search-stops text-indigo-200 max-w-[calc(100dvw)] pl-10 z-20 w-full h-12 rounded bg-neutral-900 shadow-2xl border-neutral-800/20 ring-1 ring-inset ring-neutral-700 placeholder:text-neutral-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600"
			/>
			<button
				aria-label="Clear search"
				class="z-30 w-6 h-6 text-indigo-600 hover:text-indigo-700 active:text-indigo-700 absolute right-2 my-auto top-1/2 transform -translate-y-1/2"
				on:click={() => {
					shown_bus_route_ids = bus_route_ids;
					search_term = '';
				}}
			>
				<CircleX />
			</button>
		</div>
	{/if}
</List>

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
