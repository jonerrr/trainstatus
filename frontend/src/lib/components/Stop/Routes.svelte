<script lang="ts">
	import { StopType, type RouteStop } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';

	export let routes: RouteStop[];
</script>

<div class="flex gap-1">
	{#each routes as route (route.id)}
		<Icon
			class={route.stop_type === StopType.FullTime || route.stop_type === StopType.PartTime
				? ''
				: 'opacity-30'}
			width="2rem"
			height="2rem"
			name={route.id}
		/>
	{/each}
</div>
