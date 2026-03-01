<script lang="ts">
	import { page } from '$app/state';

	import { current_time } from '$lib/url_params.svelte';

	import { ChartLine, CircleAlert, Clock, House, Settings } from '@lucide/svelte';

	interface Routes {
		[key: string]: [
			Icon: typeof House | typeof Clock | typeof CircleAlert | typeof ChartLine | typeof Settings,
			href: string
		];
	}

	const routes: Routes = {
		Home: [House, '/'],
		Alerts: [CircleAlert, '/alerts'],
		Stops: [Clock, '/stops'],
		Charts: [ChartLine, '/charts'],
		Settings: [Settings, '/settings']
	} as const;
</script>

{#snippet nav_button(label: string)}
	{@const [Icon, href] = routes[label]}
	<a
		aria-label={label}
		title={label}
		href="{href}{current_time.value ? `?at=${current_time.value}` : ''}"
		class={[
			'nav-button',
			{
				'bg-neutral-800 font-medium text-neutral-100': href === page.url.pathname,
				'text-neutral-400': href !== page.url.pathname
			}
		]}
	>
		<Icon class="nav-icon" />
		<span>{label}</span>
	</a>
{/snippet}

<!-- Mobile: fixed bottom bar (scrollable row). Desktop lg+: fixed left sidebar (column). -->
<nav
	class="fixed bottom-0 z-30 flex h-16 w-full flex-row items-stretch overflow-x-auto bg-neutral-900/95 text-center text-sm shadow-lg shadow-black/20 backdrop-blur-lg lg:top-0 lg:left-0 lg:h-full lg:w-20 lg:flex-col lg:overflow-x-hidden lg:overflow-y-auto lg:shadow-none"
>
	{@render nav_button('Home')}
	{@render nav_button('Alerts')}
	{@render nav_button('Stops')}
	{@render nav_button('Charts')}
	<div class="lg:mt-auto">
		{@render nav_button('Settings')}
	</div>
</nav>

<style>
	@reference "../app.css";

	.nav-button {
		/* Mobile: fixed-width cells so the bar scrolls rather than squishing */
		@apply flex min-w-20 flex-none flex-col items-center justify-center gap-1 p-2 transition-all duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50;
	}

	/* Desktop sidebar: buttons fill full width and get taller touch targets */
	@media (min-width: 1024px) {
		.nav-button {
			@apply w-full min-w-0 py-4;
		}
	}

	/* Gradient separator — top edge on mobile, right edge on desktop */
	nav::before {
		content: '';
		@apply absolute top-0 right-0 left-0 h-px bg-linear-to-r from-transparent via-neutral-700 to-transparent lg:top-0 lg:right-0 lg:bottom-0 lg:left-auto lg:h-full lg:w-px lg:bg-linear-to-b;
	}
</style>
