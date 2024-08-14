<script lang="ts">
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[];
	export let bus_route_ids: string[];
	export let title: string = 'Alerts';
	export let manage_height: boolean = true;

	// if (!route_ids.length) route_ids = all_route_ids;
	// if (!bus_route_ids.length) bus_route_ids = $bus_routes.map((r) => r.id);
</script>

<List class={$$props.class ?? 'max-h-[calc(100dvh-7.5rem)]'} bind:manage_height bind:title>
	<div slot="train" class="divide-y divide-neutral-800">
		{#each route_ids as route_id (route_id)}
			<Trigger {route_id} route_type="route_alert" />
		{/each}
	</div>

	<div slot="bus">
		{#each bus_route_ids as route_id (route_id)}
			<Trigger {route_id} route_type="bus_route_alert" />
		{/each}
	</div>
</List>
