<script lang="ts">
	import type { CatalogRoute } from '#lib/client/index.js';
	import { route_alerts } from '#lib/Route/alerts.js';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	interface Props {
		data: CatalogRoute;
	}

	let { data }: Props = $props();

	const transit = getTransitData();
	const current_time = getCurrentTime();

	// Rows render as their alerts arrive; a suspending read per row would batch them.
	const alerts_resource = transit.alerts(() => ({
		source: data.data.source,
		at: current_time.value
	}));
	const alerts_state = $derived(alerts_resource.current);
	const alerts = $derived(alerts_state.value && route_alerts(alerts_state.value, data.id));
</script>

<section class="flex items-center gap-1">
	<Icon height={36} width={36} link={false} route={data} />
	{#if alerts_state.error}
		<span class="text-muted">—</span>
	{:else if !alerts}
		<Skeleton lines={1} class="w-24" />
	{:else if alerts.length}
		{const alert_data = $derived(alerts[0].data)}
		<div class="font-semibold">
			{#if 'alert_type' in alert_data}
				{alert_data.alert_type}
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
