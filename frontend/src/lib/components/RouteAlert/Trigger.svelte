<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { pushState } from '$app/navigation';
	import { type Alert, type RouteAlerts, route_alerts_store } from '$lib/api';
	import { pinned_routes } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Pin from '$lib/components/Pin.svelte';
	dayjs.extend(relativeTime);

	export let route_id: string;
	// TODO: worry about sort_order

	// function on_progress(e: any) {
	// 	const [swiper, progress] = e.detail;
	// 	const index = swiper.activeIndex;
	// 	// console.log(index);
	// }

	$: route_alerts = $route_alerts_store.find((ra) => ra.route_id === route_id);
</script>

<button
	class="w-full flex justify-between items-center py-1"
	on:click={() => {
		pushState('', { dialog_open: true, dialog_id: route_id, dialog_type: 'route_alert' });
	}}
>
	<div class="flex gap-2 items-center">
		<Icon width="2rem" height="2rem" name={route_id} />

		{#if route_alerts && route_alerts.alerts.length}
			<div class="font-semibold flex gap-2 items-center">
				<div>
					{route_alerts.alerts[0].alert_type}
				</div>
			</div>
			{#if route_alerts.alerts.length > 1}
				<div class="font-normal rounded bg-indigo-200 p-1 text-neutral-800">
					+{route_alerts.alerts.length - 1}
				</div>
			{/if}
		{:else}
			<div class="text-neutral-400">No alerts</div>
		{/if}
	</div>

	<div>
		<Pin item_id={route_id} store={pinned_routes} />
	</div>
</button>
