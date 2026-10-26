<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';

	import type { ErrorProps } from './$types';

	let { error }: ErrorProps = $props();
	let retrying = $state(false);
	let retry_failed = $state(false);
	const title = $derived(error.status === 404 ? 'Page not found' : 'Something went wrong');

	async function retry() {
		retrying = true;
		retry_failed = false;
		try {
			// Kit resets its failed rendering boundaries on navigation. Refresh loads,
			// keep the current shared URL/modal, and avoid adding a history entry.
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- Retry the exact current URL, which already includes the base path.
			await goto(page.url.href, { replace: true, refreshAll: true, state: page.state });
		} catch {
			retry_failed = true;
		} finally {
			retrying = false;
		}
	}
</script>

<svelte:head>
	<title>{title} · Train Status</title>
</svelte:head>

<section class="flex min-h-full items-center justify-center bg-canvas p-6 text-primary">
	<div class="w-full max-w-md space-y-4 rounded-none border border-border-subtle bg-surface-1 p-6">
		<p class="text-sm font-medium text-muted">Error {error.status}</p>
		<h1 class="text-2xl font-semibold">{title}</h1>
		<p class="text-secondary">{error.message}</p>
		{#if retry_failed}
			<p role="alert" class="text-amber-300">Couldn’t reopen this page. Try reloading it.</p>
		{/if}
		<div class="flex flex-wrap items-center gap-4">
			{#if error.status !== 404}
				<button
					class="rounded-none bg-accent px-4 py-2 font-medium text-accent-fg hover:bg-accent-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:opacity-50"
					disabled={retrying}
					onclick={retry}>{retrying ? 'Trying again…' : 'Try again'}</button
				>
			{/if}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- Reload the exact current URL, which already includes the base path. -->
			<a class="text-accent underline underline-offset-4" href={page.url.href} data-sveltekit-reload
				>Reload page</a
			>
			<a class="text-accent underline underline-offset-4" href={resolve('/')}>Go home</a>
		</div>
	</div>
</section>
