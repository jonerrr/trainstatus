<script lang="ts">
	import { pushState } from '$app/navigation';
	import { type Route } from './static';
	import { alerts } from '$lib/alerts.svelte';
	import icons from './icons';

	const {
		route,
		link,
		express,
		class: className,
		width = 16,
		height = 16,
		show_alerts = false
	}: {
		route: Route;
		link: boolean;
		express: boolean;
		class?: string;
		width?: number;
		height?: number;
		show_alerts?: boolean;
	} = $props();

	const show_alert_icon = $derived.by(() => {
		if (!show_alerts) return false;
		// TODO: maybe differentiate between planned alerts, station notices, etc
		return alerts.alerts_by_route.has(route.id);
	});

	// const icon_name = $derived(route.route_type === 'bus' || !express ? route.id : route.id + 'X');
</script>

<!-- {#snippet alert_icon()}
	{#if show_alert_icon}
		<div class="absolute top-0 right-0 size-3 rounded-full bg-orange-400"></div>
	{/if}
{/snippet} -->

{#if route.route_type === 'bus'}
	<div
		role={link ? 'button' : undefined}
		aria-label={link ? route.short_name : undefined}
		style:background-color={`#${route.color}`}
		class="relative text-lg w-fit p-1 text-white rounded font-bold shadow-2xl flex text-center justify-center [text-shadow:_1px_1px_2px_rgb(0_0_0_/_60%),_-1px_-1px_2px_rgb(0_0_0_/_60%)] {show_alert_icon
			? 'ring-2 ring-red-800'
			: ''}"
		onclick={() => {
			if (link) pushState('', { modal: 'route', data: route });
		}}
	>
		{route.short_name}

		<!-- {@render alert_icon()} -->
	</div>
{:else}
	{@const icon_name = express ? route.id + 'X' : route.id}
	{@const icon = icons.find((i) => i.name === icon_name)!}
	<div
		role={link ? 'button' : undefined}
		aria-label={link ? route.short_name : undefined}
		class="appearance-none relative {show_alert_icon ? 'ring-2 ring-red-800 rounded-full' : ''}"
		onclick={() => {
			if (link) pushState('', { modal: 'route', data: route });
		}}
	>
		<svg class={className} {width} {height} viewBox="0 0 90 90">
			{@html icon.svg}
		</svg>

		<!-- {@render alert_icon()} -->
	</div>
{/if}

<style>
	/* the train icon has a weird white dot on webkit browsers. It goes away after hover / active or if I apply translateZ(0) */
	svg {
		transform: translateZ(0);
	}
</style>
