<script module lang="ts">
	import type { Route } from '#lib/client/index.js';

	export interface FeaturePresentation {
		route?: Route;
		title: string;
		subtitle?: string;
		detail?: string;
	}
</script>

<script lang="ts">
	import Icon from '#lib/Route/Icon.svelte';

	let { feature, link = false }: { feature: FeaturePresentation; link?: boolean } = $props();
</script>

<div
	class={[
		'flex min-w-0 gap-3 text-left',
		feature.subtitle || feature.detail ? 'items-start' : 'items-center'
	]}
	data-feature-summary
>
	{#if feature.route}
		<Icon route={feature.route} width={28} height={28} {link} />
	{/if}
	<div class="min-w-0 flex-1">
		<div class="text-sm leading-snug font-semibold wrap-anywhere text-neutral-100">
			{feature.title}
		</div>
		{#if feature.subtitle}<div class="mt-1 text-xs text-neutral-400">{feature.subtitle}</div>{/if}
		{#if feature.detail}<div class="mt-1 text-xs text-neutral-300">{feature.detail}</div>{/if}
	</div>
</div>
