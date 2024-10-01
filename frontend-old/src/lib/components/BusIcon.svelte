<script lang="ts">
	import { pushState } from '$app/navigation';
	import type { BusRoute } from '$lib/bus_api';

	export let route: BusRoute;
	export let link = true;
</script>

<button
	aria-label={`Bus Route Alerts for ${route.short_name}`}
	style={`background-color: #${route.color}`}
	class="p-1 text-indigo-100 rounded font-bold shadow-2xl"
	on:click={() => {
		if (link)
			pushState('', { dialog_open: true, dialog_id: route.id, dialog_type: 'bus_route_alert' });
	}}
>
	{route.short_name}
</button>
