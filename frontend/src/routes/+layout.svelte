<script lang="ts">
	import { onDestroy, onMount, tick, untrack } from 'svelte';

	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	import type { Catalog } from '#lib/client/index.js';
	import { setTransitData, TransitData } from '#lib/data/store.svelte.js';
	import DataBoundary from '#lib/DataBoundary.svelte';
	import Modal from '#lib/Modal.svelte';
	import Navbar from '#lib/Navbar.svelte';
	import SEO from '#lib/SEO.svelte';
	import {
		atParamToUnixSeconds,
		closeModal,
		createCurrentTime,
		modalFromUrl,
		setCurrentTime
	} from '#lib/url-params.svelte.js';

	import '@fontsource/inter';
	import '../app.css';

	let { children } = $props();

	const currentTime = setCurrentTime(
		createCurrentTime(atParamToUnixSeconds(page.url.searchParams.get('at')))
	);

	// One owner per layout instance; on the server it never fetches.
	const transit = setTransitData(
		new TransitData({
			sources: page.data.selectedSources,
			at: currentTime.value,
			eagerService: page.url.pathname !== '/map'
		})
	);
	$effect(() => {
		const sources = page.data.selectedSources;
		const eager = page.url.pathname !== '/map';
		const at = atParamToUnixSeconds(page.url.searchParams.get('at'));
		untrack(() => {
			currentTime.value = at;
			transit.setTime(at);
			transit.setEagerService(eager);
			transit.setSources(sources);
		});
	});
	onDestroy(() => transit.destroy());

	// Open the modal a direct link identifies; the modal loads its own data.
	onMount(() => {
		const modal = modalFromUrl(page.url);
		if (modal)
			void tick().then(() => {
				// eslint-disable-next-line svelte/no-navigation-without-resolve -- Preserve the current shared URL while initializing its shallow modal state.
				return goto(page.url.href, { shallow: true, replace: true, state: { modal } });
			});
		else if (['s', 'r', 't'].some((key) => page.url.searchParams.has(key)))
			closeModal(currentTime.value, true);
	});

	// CacheStorage does not perform HTTP-cache validation. The worker serves saved data
	// immediately, then sends replacements to this already-mounted app.
	onMount(() => {
		const worker = navigator.serviceWorker;
		const message = (event: MessageEvent<{ type?: string; body?: Catalog }>) => {
			if (event.data?.type === 'catalog' && event.data.body)
				transit.replaceCatalog(event.data.body);
		};
		worker?.addEventListener('message', message);
		return () => worker?.removeEventListener('message', message);
	});
</script>

<SEO />

<!-- Navbar is fixed-position; this wrapper reserves space for it.
     Mobile: pb-16 (bottom bar). Larger screens, md+: pl-20 (left sidebar). -->
<DataBoundary>
	<div class="app-shell flex h-dvh flex-col pb-16 md:pb-0 md:pl-20">
		<main class="relative min-h-0 flex-1 overflow-hidden text-white">
			<Modal />
			{@render children()}
		</main>
	</div>
	<Navbar />
</DataBoundary>

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
