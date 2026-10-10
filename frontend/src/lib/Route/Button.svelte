<script lang="ts">
	import type { Route } from '#lib/client/index.js';
	import { alert_context } from '#lib/resources/alerts.svelte.js';
	import { awaitingRows } from '#lib/resources/pending.js';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';

	interface Props {
		data: Route;
	}

	let { data }: Props = $props();

	const alerts = $derived(alert_context.getSource(data.data.source));

	const route_alerts = $derived(
		alerts?.current?.alerts_by_route
			.get(data.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === data.id)!.sort_order -
					a.entities.find((e) => e.route_id === data.id)!.sort_order
			) ?? []
	);

	const alerts_loading = $derived(awaitingRows(alerts, route_alerts.length));
</script>

<section class="flex items-center gap-1">
	<Icon height={36} width={36} link={false} route={data} />
	{#if alerts_loading}
		<Skeleton lines={1} class="w-24" />
	{:else if route_alerts.length}
		{const alert_data = $derived(route_alerts[0].data)}
		<div class="font-semibold">
			{#if 'alert_type' in alert_data}
				{alert_data.alert_type}
			{:else}
				Alert
			{/if}
		</div>
		{#if route_alerts.length > 1}
			<div class="rounded-sm bg-neutral-700 p-1 text-neutral-50">
				+{route_alerts.length - 1}
			</div>
		{/if}
	{:else}
		No Alerts
	{/if}
</section>
