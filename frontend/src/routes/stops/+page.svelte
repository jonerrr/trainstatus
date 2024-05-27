<script lang="ts">
	import VirtualList from 'svelte-tiny-virtual-list';
	import { onDestroy, onMount } from 'svelte';
	// import { slide } from 'svelte/transition';
	// import { quintOut } from 'svelte/easing';
	import { stops } from '$lib/stores';
	import { type Trip, fetch_trips } from '$lib/api';
	import List from '$lib/components/List.svelte';
	import Preview from '$lib/components/stop/Preview.svelte';

	let trips: Trip[] = [];
	let interval: number;
	let loading = true;

	// let sentinel: HTMLDivElement;
	// let observer: IntersectionObserver;

	onMount(async () => {
		// initial load
		trips = await fetch_trips(fetch);
		loading = false;

		// observer = new IntersectionObserver(
		// 	async (entries) => {
		// 		if (entries[0].isIntersecting) {
		// 			console.log('sentinel is visible');
		// 			// fetch more stops
		// 			current_stops = [
		// 				...current_stops,
		// 				...$stops.slice(current_stops.length, current_stops.length + 50)
		// 			];
		// 		}
		// 	},
		// 	{ threshold: 1 }
		// );
		// observer.observe(sentinel);

		interval = setInterval(async () => {
			console.log('fetching trips');
			// trips = await fetch_trips(fetch);
		}, 10000);
	});

	onDestroy(() => {
		console.log('clearing intervals');
		clearInterval(interval);
		// observer.disconnect();
	});

	// if virtual list doesn't work, use observer api to detect when to show ETAs
	let virtualList;
	let list_height = 0;
	console.log(list_height);
	let row_heights = [];
	for (let i = 0; i < 496; i++) {
		row_heights.push(100);
	}
</script>

<svelte:head>
	<title>Trainstat.us | Stops</title>
</svelte:head>

<!-- TODO: combine alert and stop list into one component and reuse across pages -->

<div class="p-2 text-indigo-200 text-sm" bind:offsetHeight={list_height}>
	<List bind:loading class="bg-neutral-800/90 border border-neutral-700 p-1 h-[calc(100vh-8rem)]">
		<div slot="header" class="flex self-center mb-2 w-full">
			<div class="font-semibold text-indigo-300">Stop Times</div>
		</div>

		<!-- {#each current_stops as stop (stop.id)} -->
		<!-- TODO: create a function that determines item size -->
		<VirtualList itemSize={100} bind:height={list_height} width="auto" itemCount={$stops.length}>
			<div
				slot="item"
				let:index
				let:style
				{style}
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
			>
				<Preview {trips} stop={$stops[index]} />
			</div>
		</VirtualList>
		<!-- {/each} -->

		<!-- <div bind:this={sentinel}></div> -->
	</List>
</div>
