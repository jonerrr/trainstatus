<script lang="ts">
	import type { Route } from '$lib/static';
	import { alerts as rt_alerts } from '$lib/alerts.svelte';
	import type { PersistedRune } from '$lib/util.svelte';
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';

	interface Props {
		data: Route;
		pin_rune: PersistedRune<string[]>;
	}

	let { data }: Props = $props();

	const alerts = $derived(
		rt_alerts.alerts_by_route
			.get(data.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === data.id)!.sort_order -
					a.entities.find((e) => e.route_id === data.id)!.sort_order
			) ?? []
	);
</script>

<!-- <Button state={{ modal: 'route', data }} {pin_rune}> -->
<section class="flex gap-1 items-center">
	<Icon height={36} width={36} express={false} link={true} route={data} />
	{#if alerts.length}
		<div class="font-semibold">
			{alerts[0].alert_type}
		</div>
		{#if alerts.length > 1}
			<div class="bg-neutral-700 text-neutral-50 rounded-sm p-1">
				+{alerts.length - 1}
			</div>
		{/if}
	{:else}
		No Alerts
	{/if}
</section>
<!-- </Button> -->
