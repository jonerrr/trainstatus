<script lang="ts">
	import { page } from '$app/state';

	import { getUpdateStatus } from '#lib/resources/status.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import { ChartLine, CircleAlert, Clock, House, Map, Settings } from '@lucide/svelte';

	import UpdateIndicator from './UpdateIndicator.svelte';

	const current_time = getCurrentTime();
	const updates = getUpdateStatus();

	interface Routes {
		[key: string]: [
			Icon:
				| typeof House
				| typeof Clock
				| typeof CircleAlert
				| typeof ChartLine
				| typeof Settings
				| typeof Map,
			href: string
		];
	}

	const routes: Routes = {
		Home: [House, '/'],
		Alerts: [CircleAlert, '/alerts'],
		Stops: [Clock, '/stops'],
		Charts: [ChartLine, '/charts'],
		Settings: [Settings, '/settings'],
		Map: [Map, '/map']
	} as const;
</script>

{#snippet nav_button(label: string)}
	{const [Icon, href] = $derived(routes[label])}
	<!-- use state.eager to ensure visual feedback is instant -->
	{const is_active = $derived(href === $state.eager(page.url.pathname))}
	<a
		aria-label={label}
		aria-current={is_active ? 'page' : undefined}
		title={label}
		href="{href}{current_time.value !== undefined ? `?at=${current_time.value}` : ''}"
		class={[
			'nav-button',
			{
				'bg-neutral-800 font-medium text-neutral-100': is_active,
				'text-neutral-400': !is_active
			}
		]}
	>
		<Icon class="nav-icon" />
		<span>{label}</span>
	</a>
{/snippet}

<!-- Mobile: fixed bottom bar (scrollable row). (larger screens) md+  fixed left sidebar (column). -->
<nav
	class="fixed bottom-0 z-30 flex h-16 w-full flex-col bg-neutral-900/95 text-center text-sm shadow-lg shadow-black/20 backdrop-blur-lg md:top-0 md:left-0 md:h-full md:w-20 md:shadow-none"
>
	<div class="shrink-0 md:hidden"><UpdateIndicator /></div>
	<div
		class="nav-links flex min-h-0 flex-1 items-stretch overflow-x-auto md:flex-col md:overflow-x-hidden md:overflow-y-auto"
	>
		{@render nav_button('Home')}
		{@render nav_button('Alerts')}
		{@render nav_button('Stops')}
		{@render nav_button('Charts')}
		{@render nav_button('Map')}
		{#if current_time.value !== undefined}
			<button
				type="button"
				class="nav-button"
				onclick={() => {
					void updates.retry(true);
				}}
				aria-label="Reload historical data"><Clock class="nav-icon" /><span>Reload</span></button
			>
		{/if}
		<div class="md:mt-auto">
			<div class="hidden md:block"><UpdateIndicator /></div>
			{@render nav_button('Settings')}
		</div>
	</div>
</nav>

<style>
	@reference "../app.css";

	.nav-button {
		/* Mobile: fixed-width cells so the bar scrolls rather than squishing */
		@apply flex min-w-20 flex-none flex-col items-center justify-center gap-1 p-2 transition-all duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50;
	}

	@media (max-width: 767px) {
		nav:has(:global(.update-control)) .nav-button {
			gap: 0;
			padding-block: 2px;
			font-size: 0.75rem;
			line-height: 1rem;
		}
		nav:has(:global(.update-control)) :global(.nav-icon) {
			width: 18px;
			height: 18px;
		}
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
		@apply absolute top-0 right-0 left-0 h-px bg-linear-to-r from-transparent via-neutral-700 to-transparent md:top-0 md:right-0 md:bottom-0 md:left-auto md:h-full md:w-px md:bg-linear-to-b;
	}
</style>
