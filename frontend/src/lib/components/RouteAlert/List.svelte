<script lang="ts">
	import { all_route_ids } from '$lib/api';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';
	export let manage_height: boolean = true;

	if (!route_ids.length) route_ids = all_route_ids;
</script>

<List bind:manage_height class="max-h-[calc(100dvh-8rem)]">
	<div id="list-item" class="flex text-lg justify-between self-center w-full">
		<div class="font-semibold text-indigo-300">{title}</div>
	</div>
	<div class="flex flex-col mt-1 gap-1">
		{#each route_ids as route_id (route_id)}
			<Trigger {route_id} />
		{/each}
	</div>
</List>
