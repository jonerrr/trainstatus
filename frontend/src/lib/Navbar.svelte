<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import { page } from '$app/state';
	import { current_time } from '$lib/util.svelte';

	interface Routes {
		[key: string]: [Icon: typeof Home | typeof Clock | typeof CircleAlert, href: string];
	}

	const routes: Routes = {
		Home: [Home, '/'],
		Alerts: [CircleAlert, '/alerts'],
		Stops: [Clock, '/stops']
	} as const;
</script>

{#snippet nav_button(label: string)}
	{@const [Icon, href] = routes[label]}
	<a
		aria-label={label}
		title={label}
		href="{href}{current_time.value ? `?at=${current_time.value}` : ''}"
		class="nav-button"
		class:nav-button-active={href === page.url.pathname}
		class:text-neutral-400={href !== page.url.pathname}
	>
		<Icon class="nav-icon" />
		<span>{label}</span>
	</a>
{/snippet}

<nav
	class="fixed bottom-0 h-16 w-full bg-neutral-900/95 z-30 grid grid-cols-3 items-center text-center text-sm shadow-lg shadow-black/20 backdrop-blur-lg"
>
	{@render nav_button('Home')}
	{@render nav_button('Alerts')}
	{@render nav_button('Stops')}
</nav>

<style lang="postcss">
	.nav-button {
		@apply flex flex-col items-center justify-center gap-1
			 p-2  transition-all duration-200
			 hover:bg-neutral-800/50 active:bg-neutral-700/50;
	}

	.nav-button-active {
		@apply bg-neutral-800 text-neutral-100 font-medium;
	}

	/* Gradient separator at top of nav */
	nav::before {
		content: '';
		@apply absolute top-0 left-0 right-0 h-[1px]
			 bg-gradient-to-r from-transparent via-neutral-700 to-transparent;
	}
</style>
