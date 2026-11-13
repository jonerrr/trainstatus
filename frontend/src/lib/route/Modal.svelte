<script lang="ts">
	import { onDestroy } from 'svelte';

	import type { Attachment } from 'svelte/attachments';

	import type { ApiAlert, Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import FeatureSummary from '#lib/FeatureSummary.svelte';
	import { routeAlerts as sortedAlerts } from '#lib/route/alerts.js';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';
	import { debounce } from '#lib/util.svelte.js';

	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	interface Props {
		source: Source;
		routeId: string;
		timeFormat: 'time' | 'countdown';
	}

	let { source, routeId, timeFormat }: Props = $props();

	const transit = getTransitData();
	const currentTime = getCurrentTime();
	const catalogResource = transit.catalog(() => source);
	const alertsResource = transit.alerts(() => ({ source, at: currentTime.value }));
	const catalog = $derived(catalogResource.current);
	const route = $derived(
		catalog.state === 'ready' ? catalog.value.routesById.get(routeId) : undefined
	);
	const alertsRead = $derived(alertsResource.current);
	const alerts = $derived(alertsRead.state === 'ready' ? alertsRead.value : undefined);
	const routeAlerts = $derived(sortedAlerts(alerts, routeId));

	let selectedAlertId = $state<string | null>(null);
	const idx = $derived(
		Math.max(
			0,
			routeAlerts.findIndex((alert) => alert.id === selectedAlertId)
		)
	);

	let scrollArea: HTMLDivElement | undefined;

	function observeAlert(alert: ApiAlert): Attachment<HTMLElement> {
		return (node) => {
			const observer = new IntersectionObserver(
				(entries) => {
					if (entries.some((entry) => entry.isIntersecting)) selectedAlertId = alert.id;
				},
				{ root: node.parentElement, threshold: 0.7 }
			);
			observer.observe(node);
			node.querySelectorAll('a').forEach((link) => link.removeAttribute('href'));
			return () => observer.disconnect();
		};
	}

	function scrollToAlert(i: number) {
		if (!scrollArea) return;
		const alertEls = Array.from(scrollArea.querySelectorAll('.alert')) as HTMLDivElement[];
		alertEls[i]?.scrollIntoView({ behavior: 'smooth' });
	}

	const debounceScrollToAlert = debounce(scrollToAlert);
	onDestroy(debounceScrollToAlert.cancel);
</script>

<!-- Keep the identity visible during a cold catalog load or a retryable catalog failure. -->
<header class="flex flex-col gap-3 p-3">
	{#if route}
		<FeatureSummary feature={{ route, title: route.long_name }} />
	{:else if catalog.state === 'loading'}
		<Skeleton lines={1} class="w-40" height="h-7" />
	{:else}
		<div class="text-sm font-semibold text-primary">Route {routeId}</div>
	{/if}

	{#if route}
		<div class="flex items-center gap-1 text-lg font-semibold text-primary">
			{#if alertsRead.state === 'loading'}
				<Skeleton lines={1} class="w-24" height="h-7" />
			{:else if routeAlerts.length && idx < routeAlerts.length}
				{const alertData = $derived(routeAlerts[idx].data)}
				{#if alertData.source === 'mta_subway' || alertData.source === 'mta_bus'}
					{alertData.alert_type}
				{:else}
					Alert
				{/if}
			{:else if alertsRead.state === 'ready'}
				No alerts
			{/if}
		</div>
	{/if}
</header>

<!-- handle arrow keys -->
<svelte:window
	onkeydown={(event) => {
		if (!route) return;
		if (event.key === 'ArrowLeft' && idx > 0) {
			// if we don't debounce, clicking arrow key twice really fast will get the scroll stuck
			debounceScrollToAlert(idx - 1);
		} else if (event.key === 'ArrowRight' && idx < routeAlerts.length - 1) {
			debounceScrollToAlert(idx + 1);
		}
	}}
/>

{#if catalog.state === 'ready' && !route}
	<p class="p-2 text-center text-muted">Route not found</p>
{:else if route}
	<div
		class="scrollbar-hidden flex snap-x snap-mandatory gap-2 overflow-x-scroll bg-canvas"
		{@attach (node) => {
			scrollArea = node;
			return () => {
				scrollArea = undefined;
			};
		}}
	>
		{#each routeAlerts as alert (alert.id)}
			{const header = $derived(alert.translations.find((t) => t.section === 'header'))}
			{const description = $derived(alert.translations.find((t) => t.section === 'description'))}
			<article
				{@attach observeAlert(alert)}
				class="alert flex max-h-[65dvh] w-full shrink-0 snap-start snap-always flex-col items-center justify-between gap-1"
			>
				<div class="max-h-[65dvh] overflow-auto bg-canvas px-3 py-2">
					{#if header}
						<div
							class="mb-3 text-base leading-snug font-semibold text-primary"
							role="heading"
							aria-level="2"
						>
							<!-- eslint-disable-next-line svelte/no-at-html-tags -- Alert feed markup is intentionally rendered with our static subway icons. -->
							{@html header.text}
						</div>
					{/if}
					{#if description}
						<div class="text-sm leading-relaxed text-secondary">
							<!-- eslint-disable-next-line svelte/no-at-html-tags -- Alert feed markup is intentionally rendered with our static subway icons. -->
							{@html description.text}
						</div>
					{/if}
				</div>

				{#snippet alert_time(time: number)}
					{const dt = $derived(dayjs.unix(time))}
					{#if timeFormat === 'countdown'}
						{dt.fromNow()}
					{:else if !dt.isSame(dayjs(), 'day')}
						{dt.format('h:mm A M/D')}
					{:else}
						{dt.format('h:mm A')}
					{/if}
				{/snippet}

				<div class="flex w-full justify-between px-1 text-sm text-muted">
					<div class="text-left">
						Updated:
						{@render alert_time(alert.updated_at)}
					</div>
					{#if alert.end_time != null}
						<div class="text-right">
							End:
							{@render alert_time(alert.end_time)}
						</div>
					{/if}
				</div>
			</article>
		{/each}

		{#if routeAlerts.length > 1}
			<div class="absolute bottom-0 flex w-full -translate-y-16 items-center justify-center gap-2">
				<button
					disabled={idx === 0}
					class={{ 'text-disabled': idx === 0 }}
					aria-label="Previous alert"
					onclick={() => scrollToAlert(idx - 1)}
				>
					<ChevronLeft />
				</button>
				{#each routeAlerts as alert, i (alert.id)}
					<button
						class={['size-3 rounded-none', i === idx ? 'bg-white' : 'bg-neutral-500']}
						aria-label="Scroll to alert"
						onclick={() => scrollToAlert(i)}
					>
					</button>
				{/each}
				<button
					disabled={idx === routeAlerts.length - 1}
					class={{ 'text-disabled': idx === routeAlerts.length - 1 }}
					aria-label="Next alert"
					onclick={() => scrollToAlert(idx + 1)}
				>
					<ChevronRight />
				</button>
			</div>
		{/if}
	</div>
{/if}

<style>
	.scrollbar-hidden {
		/* Chrome, Safari and Opera */
		&::-webkit-scrollbar {
			display: none;
		}
		scrollbar-width: none; /* Firefox */
		-ms-overflow-style: none; /* IE and Edge */
	}
</style>
