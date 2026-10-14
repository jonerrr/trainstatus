<script lang="ts">
	import { onDestroy } from 'svelte';

	import type { Attachment } from 'svelte/attachments';

	import type { ApiAlert, Source } from '#lib/client/index.js';
	import FeatureSummary from '#lib/FeatureSummary.svelte';
	import { route_alerts as sorted_alerts } from '#lib/Route/alerts.js';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { debounce } from '#lib/util.svelte.js';

	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	interface Props {
		source: Source;
		route_id: string;
		time_format: 'time' | 'countdown';
	}

	let { source, route_id, time_format }: Props = $props();

	const transit = getTransitData();
	const current_time = getCurrentTime();
	const catalog_resource = transit.catalog(() => source);
	const alerts_resource = transit.alerts(() => ({ source, at: current_time.value ?? null }));
	const catalog = $derived(catalog_resource.current);
	const route = $derived(catalog.value?.routes_by_id.get(route_id));
	const alerts_state = $derived(alerts_resource.current);
	const alerts = $derived(alerts_state.value);
	const route_alerts = $derived(sorted_alerts(alerts, route_id));

	let selected_alert_id = $state<string | null>(null);
	const idx = $derived(
		Math.max(
			0,
			route_alerts.findIndex((alert) => alert.id === selected_alert_id)
		)
	);

	let scroll_area: HTMLDivElement | undefined;

	function observe_alert(alert: ApiAlert): Attachment<HTMLElement> {
		return (node) => {
			const observer = new IntersectionObserver(
				(entries) => {
					if (entries.some((entry) => entry.isIntersecting)) selected_alert_id = alert.id;
				},
				{ root: node.parentElement, threshold: 0.7 }
			);
			observer.observe(node);
			node.querySelectorAll('a').forEach((link) => link.removeAttribute('href'));
			return () => observer.disconnect();
		};
	}

	function scroll_to_alert(i: number) {
		if (!scroll_area) return;
		const alert_els = Array.from(scroll_area.querySelectorAll('.alert')) as HTMLDivElement[];
		alert_els[i]?.scrollIntoView({ behavior: 'smooth' });
	}

	const debounce_scroll_to_alert = debounce(scroll_to_alert);
	onDestroy(debounce_scroll_to_alert.cancel);
</script>

<header class="flex flex-col gap-3 p-3">
	{#if route}
		<FeatureSummary feature={{ route, title: route.long_name }} />
	{:else if catalog.loading}
		<Skeleton lines={1} class="w-40" />
	{:else}
		<div class="text-sm font-semibold text-neutral-100">Route {route_id}</div>
	{/if}

	<div class="flex items-center gap-1 text-lg font-semibold text-neutral-100">
		{#if alerts_state.loading}
			<Skeleton lines={1} class="w-24" />
		{:else if route_alerts.length && idx < route_alerts.length}
			{const alert_data = $derived(route_alerts[idx].data)}
			{#if alert_data.source === 'mta_subway' || alert_data.source === 'mta_bus'}
				{alert_data.alert_type}
			{:else}
				Alert
			{/if}
		{:else if !alerts_state.error}
			No alerts
		{/if}
	</div>
</header>

<!-- handle arrow keys -->
<svelte:window
	onkeydown={(event) => {
		if (event.key === 'ArrowLeft' && idx > 0) {
			// if we don't debounce, clicking arrow key twice really fast will get the scroll stuck
			debounce_scroll_to_alert(idx - 1);
		} else if (event.key === 'ArrowRight' && idx < route_alerts.length - 1) {
			debounce_scroll_to_alert(idx + 1);
		}
	}}
/>

<div
	class="scrollbar-hidden flex snap-x snap-mandatory gap-2 overflow-x-scroll bg-neutral-950"
	{@attach (node) => {
		scroll_area = node;
		return () => {
			scroll_area = undefined;
		};
	}}
>
	{#each route_alerts as alert (alert.id)}
		{const header = $derived(alert.translations.find((t) => t.section === 'header'))}
		{const description = $derived(alert.translations.find((t) => t.section === 'description'))}
		<article
			{@attach observe_alert(alert)}
			class="alert flex max-h-[65dvh] w-full shrink-0 snap-start snap-always flex-col items-center justify-between gap-1"
		>
			<div class="max-h-[65dvh] overflow-auto bg-neutral-950 px-3 py-2">
				{#if header}
					<div
						class="mb-3 text-base leading-snug font-semibold text-neutral-100"
						role="heading"
						aria-level="2"
					>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- Alert feed markup is intentionally rendered with our static subway icons. -->
						{@html header.text}
					</div>
				{/if}
				{#if description}
					<div class="text-sm leading-relaxed text-neutral-300">
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- Alert feed markup is intentionally rendered with our static subway icons. -->
						{@html description.text}
					</div>
				{/if}
			</div>

			{#snippet alert_time(time: number)}
				{const dt = $derived(dayjs.unix(time))}
				{#if time_format === 'countdown'}
					{dt.fromNow()}
				{:else if !dt.isSame(dayjs(), 'day')}
					{dt.format('h:mm A M/D')}
				{:else}
					{dt.format('h:mm A')}
				{/if}
			{/snippet}

			<div class="flex w-full justify-between px-1 text-sm text-neutral-400">
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

	{#if route_alerts.length > 1}
		<div class="absolute bottom-0 flex w-full -translate-y-16 items-center justify-center gap-2">
			<button
				disabled={idx === 0}
				class={{ 'text-neutral-500': idx === 0 }}
				aria-label="Previous alert"
				onclick={() => scroll_to_alert(idx - 1)}
			>
				<ChevronLeft />
			</button>
			{#each route_alerts as alert, i (alert.id)}
				<button
					class={['size-3 rounded-full', i === idx ? 'bg-white' : 'bg-neutral-500']}
					aria-label="Scroll to alert"
					onclick={() => scroll_to_alert(i)}
				>
				</button>
			{/each}
			<button
				disabled={idx === route_alerts.length - 1}
				class={{ 'text-neutral-500': idx === route_alerts.length - 1 }}
				aria-label="Next alert"
				onclick={() => scroll_to_alert(idx + 1)}
			>
				<ChevronRight />
			</button>
		</div>
	{/if}
</div>

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
