<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import { page } from '$app/stores';

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
		{href}
		class="hover:bg-neutral-600 rounded"
		class:text-neutral-200={href === $page.url.pathname}
		class:text-neutral-400={href !== $page.url.pathname}
		class:font-semibold={href === $page.url.pathname}
	>
		<Icon class="m-auto h-full" />
		{label}
	</a>
	<!-- <a aria-label={label} {href} class="flex items-center justify-center h-full w-full">
		<div
			class="p-1 rounded hover:bg-neutral-600"
			class:bg-neutral-100={href === $page.url.pathname}
			class:text-neutral-400={href !== $page.url.pathname}
		>
			<Icon class="m-auto h-full" />
			{label}
		</div>
	</a> -->
{/snippet}

<nav
	class="fixed bottom-0 h-16 w-full bg-neutral-900 z-30 grid grid-cols-3 items-center text-center text-sm"
>
	{@render nav_button('Home')}
	{@render nav_button('Alerts')}
	{@render nav_button('Stops')}
</nav>
