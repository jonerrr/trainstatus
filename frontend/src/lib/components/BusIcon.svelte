<script lang="ts">
	import type { BusRoute } from '$lib/bus_api';

	export let route: BusRoute;
</script>

<span style={`background-color: #${route.color}`} class="p-1 text-indigo-100 rounded font-bold">
	{route.short_name}
</span>
