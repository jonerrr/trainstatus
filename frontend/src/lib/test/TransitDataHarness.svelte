<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/data/store.svelte.js';

	import TransitRouteReader from './TransitRouteReader.svelte';

	interface Reader {
		id: string;
		route: string;
		at?: number;
	}

	let { data, source, readers }: { data: TransitData; source: Source; readers: Reader[] } =
		$props();
</script>

{#each readers as reader (reader.id)}
	<TransitRouteReader {data} {source} routeId={reader.route} at={reader.at} />
{/each}
