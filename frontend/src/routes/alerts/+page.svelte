<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, Source } from '#lib/client/index.js';
	import List from '#lib/List.svelte';
	import { route_pins } from '#lib/pins.svelte.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { calculate_route_height } from '#lib/util.svelte.js';

	const transit = getTransitData();
	const catalogs = $derived(
		page.data.selected_sources.map((source) => ({ source, ...transit.catalogState(source) }))
	);

	// remove special express mta_subway routes (FX, 6X, 7X, etc) since they won't have any alerts
	const sources = $derived(
		Object.fromEntries(
			catalogs.map(({ source, catalog }) => [
				source,
				(catalog?.routes ?? []).filter(
					(route) => source !== 'mta_subway' || !route.id.endsWith('X')
				)
			])
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
	loading={catalogs.some((c) => !c.catalog && !c.error)}
/>
