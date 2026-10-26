<script lang="ts">
	import Icon from '#lib/Route/Icon.svelte';

	import type { FeaturePresentation } from './map/presentation';

	let {
		feature,
		link = false,
		show_alerts = false
	}: { feature: FeaturePresentation; link?: boolean; show_alerts?: boolean } = $props();
</script>

<div
	class={[
		'flex min-w-0 gap-3 text-left',
		feature.subtitle || feature.detail ? 'items-start' : 'items-center'
	]}
	data-feature-summary
>
	{#if feature.route}
		<Icon route={feature.route} width={28} height={28} {link} {show_alerts} />
	{/if}
	<div class="min-w-0 flex-1">
		<div class="text-sm leading-snug font-semibold wrap-anywhere text-primary">
			{feature.title}
		</div>
		{#if feature.subtitle}<div class="mt-1 text-xs text-muted">{feature.subtitle}</div>{/if}
		{#if feature.detail}<div class="mt-1 text-xs text-secondary">{feature.detail}</div>{/if}
	</div>
</div>
