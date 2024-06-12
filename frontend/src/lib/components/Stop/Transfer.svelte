<script lang="ts">
	import { pushState } from '$app/navigation';
	import { stops } from '$lib/stores';
	import Routes from '$lib/components/Stop/Routes.svelte';
	export let stop_id: string;

	const stop = $stops.find((s) => s.id === stop_id)!;
	console.log(stop_id);
</script>

<button
	on:click={() => pushState('', { dialog_id: stop_id, dialog_open: true, dialog_type: 'stop' })}
	class="flex rounded gap-1 border border-indigo-700 bg-indigo-800"
>
	<Routes routes={stop.routes} />
</button>
