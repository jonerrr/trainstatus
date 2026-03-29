<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import type { Route, Source } from '$lib/client';
	import { alert_context } from '$lib/resources/alerts.svelte';

	interface Props {
		data: Route;
	}

	let { data }: Props = $props();

	const alerts = $derived(alert_context.getSource(data.data.source));

	const route_alerts = $derived(
		alerts?.value?.alerts_by_route
			.get(data.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === data.id)!.sort_order -
					a.entities.find((e) => e.route_id === data.id)!.sort_order
			) ?? []
	);
	// const alerts: any[] = [];
</script>

<!-- <Button state={{ modal: 'route', data }} {pin_rune}> -->
<section class="flex items-center gap-1">
	<Icon height={36} width={36} link={true} route={data} />
	{#if route_alerts.length}
		<div class="font-semibold">
			{#if 'alert_type' in route_alerts[0].data}
				{route_alerts[0].data.alert_type}
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
<!-- </Button> -->
