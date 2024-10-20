<script lang="ts">
	import type { Route } from '$lib/static';
	import { alerts as rt_alerts } from '$lib/alerts.svelte';
	import type { PersistedRune } from '$lib/util.svelte';
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';

	interface ButtonProps {
		route: Route;
		pin_rune: PersistedRune<string[]>;
	}

	let { route, pin_rune = $bindable() }: ButtonProps = $props();

	const alerts = $derived(
		rt_alerts.alerts_by_route
			.get(route.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === route.id)!.sort_order -
					a.entities.find((e) => e.route_id === route.id)!.sort_order
			) ?? []
	);
</script>

<Button state={{ modal: 'route', data: route }} {pin_rune}>
	<section class="flex gap-1 items-center">
		<Icon height={32} width={32} express={false} link={true} {route} />
		{#if alerts.length}
			<div class="font-semibold">
				{alerts[0].alert_type}
			</div>
			{#if alerts.length > 1}
				<div class="bg-neutral-700 text-neutral-50 rounded p-1">
					+{alerts.length - 1}
				</div>
			{/if}
		{:else}
			No Alerts
		{/if}
	</section>
</Button>
