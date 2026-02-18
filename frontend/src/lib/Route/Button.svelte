<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import { alert_context } from '$lib/sources/alerts.svelte';

	import type { Route } from '@trainstatus/client';

	interface Props {
		data: Route;
	}

	let { data }: Props = $props();

	const all_alerts = alert_context.get();

	const alerts = $derived(all_alerts[data.data.source]);

	// TODO: alert stuff
	const route_alerts = $derived(
		alerts.value?.alerts_by_route
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
			{route_alerts[0].alert_type}
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
