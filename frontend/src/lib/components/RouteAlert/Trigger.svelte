<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { derived } from 'svelte/store';
	import { pinned_routes, alerts, bus_routes, pinned_bus_routes } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Pin from '$lib/components/Pin.svelte';
	import TriggerButton from '$lib/components/TriggerButton.svelte';
	import BusIcon from '../BusIcon.svelte';

	dayjs.extend(relativeTime);

	export let route_id: string;
	export let route_type: 'route_alert' | 'bus_route_alert';

	// idk if i need derived
	const route_alerts = derived(alerts, ($alerts) => {
		switch (route_type) {
			case 'route_alert':
				return $alerts
					.filter((a) => a.entities && a.entities.some((e) => e.route_id === route_id))
					.sort((a, b) => {
						return (
							b.entities.find((e) => e.route_id === route_id)!.sort_order -
							a.entities.find((e) => e.route_id === route_id)!.sort_order
						);
					});
			case 'bus_route_alert':
				return $alerts
					.filter((a) => a.entities && a.entities.some((e) => e.bus_route_id === route_id))
					.sort((a, b) => {
						return (
							b.entities.find((e) => e.bus_route_id === route_id)!.sort_order -
							a.entities.find((e) => e.bus_route_id === route_id)!.sort_order
						);
					});
		}
	});
</script>

<TriggerButton
	state={{
		dialog_open: true,
		dialog_id: route_id,
		dialog_type: route_type
	}}
>
	<div class="flex gap-2 items-center">
		{#if route_type === 'route_alert'}
			<Icon width="2rem" height="2rem" name={route_id} />
		{:else}
			<!-- TODO: simplify -->
			{@const route = $bus_routes.find((r) => r.id === route_id)}
			{#if route}
				<BusIcon {route} />
			{/if}
		{/if}

		{#if $route_alerts.length}
			<div class="font-semibold flex gap-2 items-center">
				<div>
					{$route_alerts[0].alert_type}
				</div>
			</div>
			{#if $route_alerts.length > 1}
				<div class="font-normal rounded bg-indigo-200 p-1 text-neutral-800">
					+{$route_alerts.length - 1}
				</div>
			{/if}
		{:else}
			<div class="text-neutral-400">No alerts</div>
		{/if}
	</div>

	<div>
		<Pin
			item_id={route_id}
			store={route_type === 'route_alert' ? pinned_routes : pinned_bus_routes}
		/>
	</div>
</TriggerButton>
