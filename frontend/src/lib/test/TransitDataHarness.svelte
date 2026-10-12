<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/transit/store.svelte.js';

	interface Reader {
		id: string;
		route: string;
		at?: number | null;
	}

	let { data, source, readers }: { data: TransitData; source: Source; readers: Reader[] } =
		$props();
</script>

{#each readers as reader (reader.id)}
	<svelte:boundary>
		{const route = $derived(await data.route(source, reader.route, reader.at ?? null))}
		<p data-route={reader.route}>{[...route.trips.keys()].join(',')}</p>
		{#snippet pending()}
			<p data-pending={reader.route}>loading</p>
		{/snippet}
		{#snippet failed(error)}
			<p data-failed={reader.route}>{(error as Error).message}</p>
		{/snippet}
	</svelte:boundary>
{/each}
