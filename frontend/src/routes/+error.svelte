<script lang="ts">
	import { goto } from '$app/navigation';
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

<section class="flex min-h-full items-center justify-center bg-neutral-950 p-6 text-neutral-100">
	<div class="w-full max-w-md space-y-4 rounded-xl border border-neutral-800 bg-neutral-900 p-6">
		<p class="text-sm font-medium text-neutral-400">Error {error.status}</p>
		<h1 class="text-2xl font-semibold">{title}</h1>
		<p class="text-neutral-300">{error.message}</p>
		{#if retry_failed}
			<p role="alert" class="text-amber-300">Couldn’t reopen this page. Try reloading it.</p>
		{/if}
		<div class="flex flex-wrap items-center gap-4">
			{#if error.status !== 404}
				<button
					class="rounded-md bg-blue-600 px-4 py-2 font-medium hover:bg-blue-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-400 disabled:opacity-50"
					disabled={retrying}
					onclick={retry}>{retrying ? 'Trying again…' : 'Try again'}</button
				>
			{/if}
			<a
				class="text-blue-300 underline underline-offset-4"
				href={page.url.href}
				data-sveltekit-reload>Reload page</a
			>
			<a class="text-blue-300 underline underline-offset-4" href="/">Go home</a>
		</div>
	</div>
</section>
