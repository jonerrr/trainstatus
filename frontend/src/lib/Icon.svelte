<script lang="ts">
	import { pushState } from '$app/navigation';
	import { type Route } from './static';
	import icons from './icons';

	const {
		route,
		link,
		express,
		class: className,
		width = 16,
		height = 16
	}: {
		route: Route;
		link: boolean;
		express: boolean;
		class?: string;
		width?: number;
		height?: number;
	} = $props();
</script>

{#if route.route_type === 'bus'}
	<div
		role={link ? 'button' : undefined}
		aria-label={link ? route.short_name : undefined}
		style:background-color={`#${route.color}`}
		class="text-lg w-fit p-1 text-white rounded font-bold shadow-2xl flex text-center justify-center [text-shadow:_1px_1px_2px_rgb(0_0_0_/_60%),_-1px_-1px_2px_rgb(0_0_0_/_60%)]"
		onclick={() => {
			if (link) pushState('', { modal: 'route', data: route });
		}}
	>
		{route.short_name}
	</div>
{:else}
	{@const icon_name = express ? route.id + 'X' : route.id}
	{@const icon = icons.find((i) => i.name === icon_name)!}
	<div
		role={link ? 'button' : undefined}
		aria-label={link ? route.short_name : undefined}
		class="appearance-none"
		onclick={() => {
			if (link) pushState('', { modal: 'route', data: route });
		}}
	>
		<svg class={className} {width} {height} viewBox="0 0 90 90">
			{@html icon.svg}
		</svg>
	</div>
{/if}

<style>
	/* the train icon has a weird white dot on webkit browsers. It goes away after hover / active or if I apply translateZ(0) */
	svg {
		transform: translateZ(0);
	}
</style>
