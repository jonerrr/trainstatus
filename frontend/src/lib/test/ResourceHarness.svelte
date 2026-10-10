<script lang="ts">
	import { untrack } from 'svelte';

	import '../../app.css';

	import type { Stop } from '../client';
	import List from '../List.svelte';
	import Navbar from '../Navbar.svelte';
	import { alert_context } from '../resources/alerts.svelte';
	import { LiveResource } from '../resources/index.svelte';
	import { resourceQuery } from '../resources/request';
	import { setUpdateStatus, UpdateStatus } from '../resources/status.svelte';
	import { stop_time_context, StopTimeLiveResource } from '../resources/stop_times.svelte';
	import { trip_context } from '../resources/trips.svelte';
	import { createCurrentTime, setCurrentTime } from '../url_params.svelte';
	import type { ResourceControls } from './resourceControls';

	let {
		fetcher,
		routeFetcher,
		onready,
		stops = []
	}: {
		fetcher: (at: number | null, signal: AbortSignal) => Promise<string[]>;
		routeFetcher: typeof fetch;
		onready: (controls: ResourceControls) => void;
		stops?: Stop[];
	} = $props();
	const time = setCurrentTime(createCurrentTime());
	const status = setUpdateStatus(new UpdateStatus());
	trip_context.set({});
	alert_context.set({});
	const resource = new LiveResource<string[]>((query, signal) => fetcher(query.at, signal), [], {
		query: () => resourceQuery('mta_subway', 'trips', time.value ?? null),
		interval: 0,
		debounce: 20
	});
	const arrivals = new StopTimeLiveResource(
		'mta_bus',
		untrack(() => routeFetcher)
	);
	stop_time_context.set({ mta_bus: arrivals });
	untrack(() => onready({ resource, arrivals, time, status }));
</script>

<main data-content class="h-40">
	<p data-values>{resource.current.join(',')}</p>
</main>
{#if stops.length}
	<List
		title="Stops"
		type="stop"
		sources={{ mta_bus: stops }}
		height_calc={() => 100}
		container_class="h-[300px]"
	/>
{/if}
<Navbar />
