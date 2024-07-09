<script lang="ts">
	import { StopType, type RouteStop } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';

	export let routes: RouteStop[];
	export let link: boolean = false;

	// sort routes so part time are last
	const routes_s = routes.sort((a, b) => {
		if (a.stop_type === StopType.PartTime && b.stop_type !== StopType.PartTime) {
			return -1;
		}
		if (a.stop_type !== StopType.PartTime && b.stop_type === StopType.PartTime) {
			return 1;
		}
		return 0;
	});
</script>

<div class="flex gap-1">
	{#each routes_s as route (route.id)}
		<Icon
			class={route.stop_type === StopType.FullTime || route.stop_type === StopType.PartTime
				? ''
				: 'opacity-30'}
			width="2rem"
			height="2rem"
			name={route.id}
			{link}
		/>
	{/each}
</div>
