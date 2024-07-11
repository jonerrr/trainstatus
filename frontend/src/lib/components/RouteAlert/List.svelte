<script lang="ts">
	import { all_route_ids } from '$lib/api';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';
	export let manage_height: boolean = true;

	if (!route_ids.length) route_ids = all_route_ids;
</script>

<List class={$$props.class ?? 'max-h-[calc(100dvh-7.5rem)]'} bind:manage_height bind:title>
	<div slot="train" class="divide-y divide-neutral-800">
		{#each route_ids as route_id (route_id)}
			<Trigger {route_id} />
		{/each}
	</div>

	<div slot="bus">Bus alerts coming soon</div>
</List>
