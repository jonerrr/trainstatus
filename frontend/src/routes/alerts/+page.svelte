<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { route_pins } from '$lib/pins.svelte';

	// TODO: combine express and local
</script>

<List
	title="Route Alerts"
	sources={page.data.routes}
	type="route"
	pins={route_pins}
	class="max-h-[calc(100dvh-10.5rem)]"
	height_calc={() => 45}
	ssr_min={20}
/>
