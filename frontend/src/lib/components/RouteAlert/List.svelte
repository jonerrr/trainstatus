<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import List from '$lib/components/List.svelte';
	import RouteAlertDialog from '$lib/components/RouteAlert/Dialog.svelte';
	import { type RouteAlerts, route_alerts_store } from '$lib/api_new';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';

	let routes: RouteAlerts[] = [
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
	// get only wanted route_ids
	if (route_ids.length) routes = routes.filter((route) => route_ids.includes(route.route_id));
</script>

<div
	class={`overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 min-h-[30%] max-h-[calc(100vh-8rem)]`}
>
	<div class="flex text-lg self-center mb-2 w-full">
		<div class="font-semibold text-indigo-300">{title}</div>
	</div>

	{#each routes as route_alerts (route_alerts.route_id)}
		<div
			class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
			transition:slide={{ easing: quintOut, axis: 'y' }}
		>
			<RouteAlertDialog route_id={route_alerts.route_id} />
		</div>
	{/each}
</div>
