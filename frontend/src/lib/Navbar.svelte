<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { RouteId } from '$app/types';

	import { getCurrentTime } from '#lib/url-params.svelte.js';

	import { ChartLine, CircleAlert, Clock, House, Map, Settings } from '@lucide/svelte';

	import UpdateIndicator from './UpdateIndicator.svelte';

	const currentTime = getCurrentTime();

	type NavIcon =
		| typeof House
		| typeof Clock
		| typeof CircleAlert
		| typeof ChartLine
		| typeof Settings
		| typeof Map;

	interface NavItem {
		label: string;
		icon: NavIcon;
		href: RouteId;
	}

	const navItems: readonly NavItem[] = [
		{ label: 'Home', icon: House, href: '/' },
		{ label: 'Alerts', icon: CircleAlert, href: '/alerts' },
		{ label: 'Stops', icon: Clock, href: '/stops' },
		{ label: 'Charts', icon: ChartLine, href: '/charts' },
		{ label: 'Map', icon: Map, href: '/map' },
		{ label: 'Settings', icon: Settings, href: '/settings' }
	];
</script>

{#snippet navButton(item: NavItem)}
	{const Icon = $derived(item.icon)}
	{const href = $derived(item.href)}
	{const label = $derived(item.label)}
	<!-- use state.eager to ensure visual feedback is instant -->
	{const isActive = $derived(
		new URL(resolve(href), page.url.href).pathname === $state.eager(page.url.pathname)
	)}
	<a
		aria-label={label}
		aria-current={isActive ? 'page' : undefined}
		title={label}
		href="{resolve(href)}{currentTime.value !== undefined ? `?at=${currentTime.value}` : ''}"
		class={[
			'nav-button',
			{
				'bg-accent-bg font-medium text-accent': isActive,
				'text-muted': !isActive
			}
		]}
	>
		<Icon class="nav-icon" />
		<span>{label}</span>
	</a>
{/snippet}

<!-- Mobile: fixed bottom bar (scrollable row). (larger screens) md+  fixed left sidebar (column). -->
<nav
	class="fixed bottom-0 z-30 flex h-16 w-full flex-col bg-surface-1/95 text-center text-sm shadow-lg shadow-black/20 backdrop-blur-lg md:top-0 md:left-0 md:h-full md:w-20 md:shadow-none"
>
	{#if !page.state.modal}<div class="shrink-0 md:hidden"><UpdateIndicator /></div>{/if}
	<div
		class="nav-links flex min-h-0 flex-1 items-stretch overflow-x-auto md:flex-col md:overflow-x-hidden md:overflow-y-auto"
	>
		{#each navItems.slice(0, 5) as item (item.href)}
			{@render navButton(item)}
		{/each}
		<div class="md:mt-auto">
			{#if !page.state.modal}<div class="hidden md:block"><UpdateIndicator /></div>{/if}
			{@render navButton(navItems[5])}
		</div>
	</div>
</nav>

<style>
	@reference "../app.css";

	.nav-button {
		/* Mobile: fixed-width cells so the bar scrolls rather than squishing */
		@apply flex min-w-20 flex-none flex-col items-center justify-center gap-1 p-2 transition-all duration-200 hover:bg-hover/50 active:bg-border-default/50;
	}

	.nav-button[aria-current='page'] {
		box-shadow: inset 0 2px var(--accent);
	}

	@media (min-width: 768px) {
		.nav-button[aria-current='page'] {
			box-shadow: inset 2px 0 var(--accent);
		}
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
		@apply absolute top-0 right-0 left-0 h-px bg-linear-to-r from-transparent via-border-default to-transparent md:top-0 md:right-0 md:bottom-0 md:left-auto md:h-full md:w-px md:bg-linear-to-b;
	}
</style>
