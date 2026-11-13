<script lang="ts">
	import type { CatalogRoute } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import { routeAlerts } from '#lib/route/alerts.js';
	import Icon from '#lib/route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';

	interface Props {
		data: CatalogRoute;
	}

	let { data }: Props = $props();

	const transit = getTransitData();
	const currentTime = getCurrentTime();

	// Rows render as their alerts arrive; a suspending read per row would batch them.
	const alertsResource = transit.alerts(() => ({
		source: data.data.source,
		at: currentTime.value
	}));
	const alertsRead = $derived(alertsResource.current);
	const alerts = $derived(
		alertsRead.state === 'ready' ? routeAlerts(alertsRead.value, data.id) : undefined
	);
</script>

<section class="flex items-center gap-1">
	<Icon height={36} width={36} link={false} route={data} />
	{#if alertsRead.state === 'failed'}
		<span class="text-muted">—</span>
	{:else if !alerts}
		<Skeleton lines={1} class="w-24" height="h-6" />
	{:else if alerts.length}
		{const alertData = $derived(alerts[0].data)}
		<div class="font-semibold">
			{#if 'alert_type' in alertData}
				{alertData.alert_type}
			{:else}
				Alert
			{/if}
		</div>
		{#if alerts.length > 1}
			<div class="rounded-none bg-border-default p-1 text-primary">
				+{alerts.length - 1}
			</div>
		{/if}
	{:else}
		No Alerts
	{/if}
</section>
