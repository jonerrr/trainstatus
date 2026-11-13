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

<!-- Mobile: six equal columns. Larger screens, md+: fixed left sidebar. -->
<nav
	class="fixed bottom-0 z-30 flex h-16 w-full flex-col border-t border-border-default bg-surface-1 text-center text-sm md:top-0 md:left-0 md:h-full md:w-20 md:border-t-0 md:border-r"
>
	{#if !page.state.modal}<div class="shrink-0 md:hidden"><UpdateIndicator /></div>{/if}
	<div
		class="nav-links grid min-h-0 flex-1 grid-cols-6 items-stretch md:flex md:flex-col md:overflow-x-hidden md:overflow-y-auto"
	>
		{#each navItems.slice(0, 5) as item (item.href)}
			{@render navButton(item)}
		{/each}
		<div class="flex min-w-0 md:mt-auto md:block">
			{#if !page.state.modal}<div class="hidden md:block"><UpdateIndicator /></div>{/if}
			{@render navButton(navItems[5])}
		</div>
	</div>
</nav>

<style>
	.nav-button {
		display: flex;
		min-width: 0;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.25rem;
		padding: 0.5rem 0.25rem;
		font-size: 0.75rem;
		line-height: 1rem;
		transition: all 200ms var(--default-transition-timing-function);
	}

	@media (hover: hover) {
		.nav-button:hover {
			background-color: color-mix(in oklab, var(--bg-hover) 50%, transparent);
		}
	}

	.nav-button:active {
		background-color: color-mix(in oklab, var(--border-default) 50%, transparent);
	}

	.nav-links > div .nav-button {
		width: 100%;
	}

	.nav-button :global(.nav-icon) {
		flex-shrink: 0;
	}

	.nav-button[aria-current='page'] {
		box-shadow: inset 0 2px var(--accent);
	}

	@media (min-width: 768px) {
		.nav-button {
			min-width: 5rem;
			flex: none;
			padding: 0.5rem;
			font-size: 0.875rem;
			line-height: 1.25rem;
		}

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
			width: 100%;
			min-width: 0;
			padding-block: 1rem;
		}
	}
</style>
