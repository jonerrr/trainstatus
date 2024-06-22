<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { all_route_ids } from '$lib/api';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';
	export let manage_height: boolean = true;

	if (!route_ids.length) route_ids = all_route_ids;
</script>

<List bind:manage_height class="max-h-[calc(100dvh-8rem)]">
	<div class="flex text-lg justify-between self-center w-full">
		<div class="font-semibold text-indigo-300">{title}</div>
	</div>
	<div class="flex flex-col mt-1 gap-1">
		{#each route_ids as route_id, i (route_id)}
			<div
				id="list-item"
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1"
				transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
			>
				<Trigger {route_id} />
			</div>
		{/each}
	</div>
</List>
