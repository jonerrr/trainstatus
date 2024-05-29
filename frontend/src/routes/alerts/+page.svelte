<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import List from '$lib/components/List.svelte';
	import RoutePreview from '$lib/components/route/Preview.svelte';
	import { onMount } from 'svelte';
	import { fetch_alerts, type RouteAlerts } from '$lib/api';
	// import type { PageData } from './$types';

	// export let data: PageData;

	// default set of routes for alerts
	// const routes = [
	// 	'1',
	// 	'2',
	// 	'3',
	// 	'4',
	// 	'5',
	// 	'6',
	// 	'7',
	// 	'A',
	// 	'C',
	// 	'E',
	// 	'B',
	// 	'D',
	// 	'F',
	// 	'M',
	// 	'G',
	// 	'J',
	// 	'Z',
	// 	'L',
	// 	'N',
	// 	'Q',
	// 	'R',
	// 	'W',
	// 	'H',
	// 	'FS',
	// 	'GS',
	// 	'SI'
	// ];

	const routes: RouteAlerts[] = [
		{
			route_id: '1',
			alerts: []
		},
		{
			route_id: '2',
			alerts: []
		},
		{
			route_id: '3',
			alerts: []
		},
		{
			route_id: '4',
			alerts: []
		},
		{
			route_id: '5',
			alerts: []
		},
		{
			route_id: '6',
			alerts: []
		},
		{
			route_id: '7',
			alerts: []
		},
		{
			route_id: 'A',
			alerts: []
		},
		{
			route_id: 'C',
			alerts: []
		},
		{
			route_id: 'E',
			alerts: []
		},
		{
			route_id: 'B',
			alerts: []
		},
		{
			route_id: 'D',
			alerts: []
		},
		{
			route_id: 'F',
			alerts: []
		},
		{
			route_id: 'M',
			alerts: []
		},
		{
			route_id: 'G',
			alerts: []
		},
		{
			route_id: 'J',
			alerts: []
		},
		{
			route_id: 'Z',
			alerts: []
		},
		{
			route_id: 'L',
			alerts: []
		},
		{
			route_id: 'N',
			alerts: []
		},
		{
			route_id: 'Q',
			alerts: []
		},
		{
			route_id: 'R',
			alerts: []
		},
		{
			route_id: 'W',
			alerts: []
		},
		{
			route_id: 'H',
			alerts: []
		},
		{
			route_id: 'FS',
			alerts: []
		},
		{
			route_id: 'GS',
			alerts: []
		},
		{
			route_id: 'SI',
			alerts: []
		}
	];

	let loading = true;

	onMount(async () => {
		// fetch alerts
		const rt_alerts = await fetch_alerts(fetch);
		// put rt_alerts into route_alerts
		rt_alerts.forEach((alert) => {
			const i = routes.findIndex((route) => route.route_id === alert.route_id);
			routes[i].alerts.push(...alert.alerts);
		});
		loading = false;
		// TODO: auto reload
	});
</script>

<svelte:head>
	<title>Trainstat.us | Alerts</title>
</svelte:head>

<!-- TODO: combine alert and stop list into one component and reuse across pages -->
<!-- TODO: show routes even when there is no alert for them -->

<div class="p-2 text-indigo-200 text-sm">
	<List bind:loading class="bg-neutral-800/90 border border-neutral-700 p-1 h-[calc(100vh-8rem)]">
		<div slot="header" class="flex self-center mb-2 w-full">
			<div class="font-semibold text-indigo-300">Alerts</div>
		</div>

		{#each routes as route_alerts (route_alerts.route_id)}
			<div
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
				transition:slide={{ easing: quintOut, axis: 'y' }}
			>
				<RoutePreview {route_alerts} route_id={route_alerts.route_id} />
			</div>
		{/each}
	</List>
</div>
