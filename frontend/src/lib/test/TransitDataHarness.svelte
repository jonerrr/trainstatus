<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/transit/store.svelte.js';

	import TransitRouteReader from './TransitRouteReader.svelte';

	interface Reader {
		id: string;
		route: string;
		at?: number | null;
	}

	let { data, source, readers }: { data: TransitData; source: Source; readers: Reader[] } =
		$props();
</script>

{#each readers as reader (reader.id)}
	<TransitRouteReader {data} {source} route_id={reader.route} at={reader.at ?? null} />
{/each}
