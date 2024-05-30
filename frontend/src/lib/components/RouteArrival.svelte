<script lang="ts">
	import { liveQuery } from 'dexie';
	import { db } from '$lib/db';

	$: stop_times = liveQuery(async () => db.stop_time.where('stop_id').equals('250').toArray());
</script>

<div class="text-white">
	{#if $stop_times}
		{#each $stop_times as stop_time}
			<!-- <p>{new Date(stop_time.arrival).toLocaleString()}</p> -->
			<p>{stop_time.arrival.toLocaleString()}</p>
		{/each}
	{/if}
</div>
