<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, Source } from '#lib/client/index.js';
	import List from '#lib/List.svelte';
	import { route_pins } from '#lib/pins.svelte.js';
	import { alert_routes } from '#lib/Route/alerts.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { calculate_route_height } from '#lib/util.svelte.js';

	const transit = getTransitData();
	const catalog_resources = transit.catalogs(() => page.data.selected_sources);
	const catalogs = $derived(catalog_resources.current);

	const sources = $derived(
		Object.fromEntries(
			catalogs.map(({ source, catalog }) => [source, catalog ? alert_routes(catalog) : []])
		) as Partial<Record<Source, CatalogRoute[]>>
	);
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
