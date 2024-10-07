<script lang="ts">
	import { pushState } from '$app/navigation';
	import { type Route } from './static';
	import icons from './icons';

	const {
		route,
		link,
		express,
		class: className,
		width = '1rem',
		height = '1rem'
	}: {
		route: Route;
		link: boolean;
		express: boolean;
		class?: string;
		width?: string;
		height?: string;
	} = $props();
</script>

{#if route.route_type === 'bus'}
	<div
		role={link ? 'button' : undefined}
		style:background-color={`#${route.color}`}
		class="p-1 text-indigo-100 rounded font-bold shadow-2xl"
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
