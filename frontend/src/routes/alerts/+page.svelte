<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import List from '#lib/List.svelte';
	import { routePins } from '#lib/pins.svelte.js';
	import { alertRoutes } from '#lib/route/alerts.js';
	import { calculateRouteHeight } from '#lib/util.svelte.js';

	const transit = getTransitData();
	const catalogResources = transit.catalogs(() => page.data.selectedSources);
	const catalogs = $derived(catalogResources.current.filter((r) => r.state === 'ready'));

	const sources = $derived(
		Object.fromEntries(
			catalogs.map(({ source, value }) => [source, alertRoutes(value)])
		) as Partial<Record<Source, CatalogRoute[]>>
	);
</script>

<List
	title="Route Alerts"
	{sources}
	type="route"
	pins={routePins}
	containerClass="h-full"
	heightCalc={calculateRouteHeight}
	ssrMin={20}
/>
