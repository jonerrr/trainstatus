<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';

	// interface NavbarButtonProps {
	// 	href?: string;
	// 	label?: string;
	// }

	// let { children, href = '/', label = 'Home' }: NavbarButtonProps = $props();

	let {
		children,
		href = '/',
		label = 'Home'
	}: { children: Snippet; href: string; label: string } = $props();

	// const at = $page.url.searchParams.get('at');
	// if (at) {
	// 	href += `?at=${at}`;
	// }
</script>

<!-- adding pb so navbar on ios isn't in the way -->
<!-- href={at ? `${href}?at=${at}` : href} -->
<a
	aria-label={label}
	{href}
	class="grid h-full w-full grid-cols-1 text-center rounded-none text-sm"
	class:text-neutral-200={href === $page.url.pathname}
	class:text-neutral-400={href !== $page.url.pathname}
	class:underline={href === $page.url.pathname}
>
	{@render children()}
	{label}
</a>
