<script lang="ts">
	import { onDestroy, onMount, tick, untrack } from 'svelte';

	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	import type { Catalog } from '#lib/client/index.js';
	import Modal from '#lib/Modal.svelte';
	import Navbar from '#lib/Navbar.svelte';
	import SEO from '#lib/SEO.svelte';
	import { syncLocalStorage } from '#lib/storage.svelte.js';
	import { setTransitData, TransitData } from '#lib/transit/store.svelte.js';
	import {
		atParamToUnixSeconds,
		close_modal,
		createCurrentTime,
		modal_from_url,
		setCurrentTime
	} from '#lib/url_params.svelte.js';

	import '@fontsource/inter';
	import '../app.css';

	let { children } = $props();

	const current_time = setCurrentTime(createCurrentTime(atParamToUnixSeconds(page.data.at)));

	// One owner per layout instance; on the server it never fetches.
	const transit = setTransitData(
		new TransitData({
			sources: page.data.selected_sources,
			eager_service: page.url.pathname !== '/map'
		})
	);
	$effect(() => {
		const sources = page.data.selected_sources;
		const eager = page.url.pathname !== '/map';
		untrack(() => {
			transit.setEagerService(eager);
			transit.setSources(sources);
		});
	});
	onDestroy(() => transit.destroy());
	onMount(syncLocalStorage);

	// Open the modal a direct link identifies; the modal loads its own data.
	onMount(() => {
		const modal = modal_from_url(page.url);
		if (modal) void tick().then(() => goto('', { shallow: true, replace: true, state: { modal } }));
		else if (['s', 'r', 't'].some((key) => page.url.searchParams.has(key)))
			close_modal(current_time.value, true);
	});

	// The service worker posts catalogs that changed after it served a saved copy.
	onMount(() => {
		const worker = navigator.serviceWorker;
		const message = (event: MessageEvent<{ type?: string; body?: Catalog }>) => {
			if (event.data?.type === 'catalog' && event.data.body)
				transit.replaceCatalog(event.data.body);
		};
		worker?.addEventListener('message', message);
		return () => worker?.removeEventListener('message', message);
	});

	// Sync current_time.value with ?at URL param whenever it changes.
	// Reads page.url inside untrack so this effect only re-runs when current_time changes.
	$effect(() => {
		const val = current_time.value;
		untrack(() => {
			const current_at = page.url.searchParams.get('at');
			const new_at = val !== undefined ? val.toString() : null;
			console.log(
				`current_time changed: ${current_time.value} (URL param at=${current_at}), syncing to ${new_at}`
			);
			if (current_at === new_at) return; // no change needed

			const url = new URL(page.url.href);
			if (val !== undefined) url.searchParams.set('at', val.toString());
			else url.searchParams.delete('at');
			console.log(`Updating URL search param at=${new_at} (was ${current_at})`);
			goto(url.pathname + url.search, { shallow: true, replace: true, state: page.state });
		});
	});
</script>

<SEO />

<svelte:window
	ononline={() => {
		transit.retry();
	}}
/>

<!-- Navbar is fixed-position; this wrapper reserves space for it.
     Mobile: pb-16 (bottom bar). Larger screens, md+: pl-20 (left sidebar). -->
<div class="app-shell flex h-dvh flex-col pb-16 md:pb-0 md:pl-20">
	<main class="relative min-h-0 flex-1 overflow-hidden text-white">
		<Modal />
		{@render children()}
	</main>
</div>
<Navbar />

<style>
	:global(body) {
		background-color: var(--color-neutral-900);
	}

	/* Android Chrome sizes 100dvh too tall in an installed PWA after a
	   pull-to-refresh reload, which makes the document scroll. svh matches
	   the visible viewport in standalone mode. */
	@media (display-mode: standalone), (display-mode: fullscreen) {
		.app-shell {
			height: 100svh;
			max-height: 100svh;
		}
	}
</style>
