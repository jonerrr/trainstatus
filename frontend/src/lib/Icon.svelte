<script lang="ts">
	import { type Route, RouteType } from './static';
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
		width: string;
		height: string;
	} = $props();

	const icon_name = $derived.by(() => {
		return express ? route.id + 'X' : route.id;
	});
	const icon = $derived(icons.find((i) => i.name === icon_name)!);
</script>

{#if route.route_type === RouteType.Bus}
	<button
		style:background-color={`#${route.color}`}
		class="p-1 text-indigo-100 rounded font-bold shadow-2xl"
		onclick={() => {
			console.log('TODO: implement pushState');
		}}
	>
		{route.short_name}
	</button>
{:else}
	<button class="appearance-none">
		<svg class={className} {width} {height} viewBox="0 0 90 90">
			{@html icon.svg}
		</svg>
	</button>
{/if}
