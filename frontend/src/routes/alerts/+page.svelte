<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { route_pins } from '$lib/pins.svelte';

	// remove special express mta_subway routes (FX, 6X, 7X, etc) since they won't have any alerts
	const sources = $derived({
		...page.data.routes,
		mta_subway: page.data.routes['mta_subway'].filter((route) => !route.id.endsWith('X'))
	});
</script>

<List
	title="Route Alerts"
	{sources}
	type="route"
	pins={route_pins}
	class="max-h-[calc(100dvh-10.5rem)]"
	height_calc={() => 45}
	ssr_min={20}
/>
