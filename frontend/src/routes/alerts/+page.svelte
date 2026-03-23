<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { route_pins } from '$lib/pins.svelte';
	import { calculate_route_height } from '$lib/util.svelte';

	import type { Route, Source } from '@trainstatus/client';

	// remove special express mta_subway routes (FX, 6X, 7X, etc) since they won't have any alerts
	const sources = $derived({
		...page.data.routes,
		mta_subway: page.data.routes['mta_subway']?.filter((route) => !route.id.endsWith('X'))
	});
</script>

<List
	title="Route Alerts"
	{sources}
	type="route"
	pins={route_pins}
	container_class="h-full"
	height_calc={calculate_route_height}
	ssr_min={20}
/>
