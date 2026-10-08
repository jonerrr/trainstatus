<script lang="ts">
	import type { Route } from '#lib/client/index.js';
	import icons from '#lib/icons.js';
	import { alert_context } from '#lib/resources/alerts.svelte.js';
	import { open_modal } from '#lib/url_params.svelte.js';

	let {
		route,
		link,
		class: className,
		width = 16,
		height = 16,
		show_alerts = false
	}: {
		route: Route;
		link: boolean;
		class?: string;
		width?: number;
		height?: number;
		show_alerts?: boolean;
	} = $props();
	const alerts = alert_context.get();
	const hasAlert = $derived(
		show_alerts && alerts?.[route.data.source]?.current?.alerts_by_route.has(route.id)
	);
	const classes = $derived([
		'inline-flex shrink-0 items-center rounded-md',
		hasAlert && 'ring-2 ring-orange-400',
		className
	]);
	const size = $derived(Math.max(width, height));
	const icon = $derived(
		route.data.source === 'mta_subway' ? icons.find((item) => item.name === route.id) : undefined
	);
	const label = $derived(route.short_name);
	const parts = $derived(route.data.source === 'mta_bus' ? label.match(/^([A-Za-z]+)(.*)$/) : null);
</script>

{#snippet badge()}
	{#if icon}
		<svg
			class="route-badge shrink-0"
			width={size}
			height={size}
			viewBox="0 0 90 90"
			role="img"
			aria-label={label}
		>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -- Trusted static subway artwork. -->
			{@html icon.svg}
		</svg>
	{:else}
		<span
			class="route-badge bus-badge"
			style:--badge-size={`${size}px`}
			style:background-color={route.color || '#526173'}
			style:color={route.text_color || 'white'}
			aria-label={label}
		>
			{#if parts?.[2]}<span class="prefix">{parts[1]}</span>{parts[2]}{:else}{label}{/if}
		</span>
	{/if}
{/snippet}

{#if link}
	<button
		type="button"
		class={[classes, 'focus-visible:outline-2 focus-visible:outline-blue-400']}
		aria-label={`Open route ${label}`}
		onclick={() => open_modal({ type: 'route', ...route })}
	>
		{@render badge()}
	</button>
{:else}
	<span class={classes}>{@render badge()}</span>
{/if}

<style>
	.route-badge {
		flex-shrink: 0;
	}
	svg {
		transform: translateZ(0);
	}
	.bus-badge {
		display: inline-flex;
		align-items: baseline;
		justify-content: center;
		gap: 1px;
		min-width: var(--badge-size);
		min-height: var(--badge-size);
		padding: 0.2em 0.45em;
		border-radius: 0.4rem;
		font-size: calc(var(--badge-size) * 0.52);
		line-height: 1.4;
		font-weight: 750;
		white-space: nowrap;
		text-shadow: 0 1px 2px #0009;
	}
	.prefix {
		font-size: 0.78em;
	}
</style>
