<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import List from '$lib/components/List.svelte';
	import RoutePreview from '$lib/components/route/Preview.svelte';
	import type { PageData } from './$types';

	export let data: PageData;
</script>

<svelte:head>
	<title>Trainstat.us | Alerts</title>
</svelte:head>

<!-- TODO: combine alert and stop list into one component and reuse across pages -->

<div class="p-2 text-indigo-200 text-sm">
	{#await data.alerts}
		{#each Array(20) as _}
			<div class="flex flex-col gap-2">
				<div
					class="flex rounded w-full animate-pulse items-center justify-center my-1 h-10 bg-neutral-700"
				></div>
			</div>
		{/each}
	{:then alerts}
		<List
			loading={false}
			class="bg-neutral-800/90 border border-neutral-700 p-1 h-[calc(100vh-8rem)]"
		>
			<div slot="header" class="flex self-center mb-2 w-full">
				<div class="font-semibold text-indigo-300">Alerts</div>
			</div>

			{#each alerts as route_alerts (route_alerts.route_id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<RoutePreview {route_alerts} route_id={route_alerts.route_id} />
				</div>
			{/each}
		</List>
	{:catch error}
		<p>{error.message}</p>
	{/await}
</div>
