<script lang="ts">
	import type { Attachment } from 'svelte/attachments';

	import Icon from '$lib/Icon.svelte';
	import { alert_context } from '$lib/sources/alerts.svelte';
	import { debounce } from '$lib/util.svelte';

	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import type { Route } from '@trainstatus/client';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	interface Props {
		route: Route;
		time_format: 'time' | 'countdown';
	}

	let { route, time_format }: Props = $props();

	const all_alerts = alert_context.get();

	const alerts = $derived(all_alerts[route.data.source]);

	const route_alerts = $derived(
		alerts.value?.alerts_by_route
			.get(route.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === route.id)!.sort_order -
					a.entities.find((e) => e.route_id === route.id)!.sort_order
			) ?? []
	);

	let idx = $state(0);

	let scroll_area: HTMLDivElement | undefined;

	const manage_scroll: Attachment = (node) => {
		const observer = new IntersectionObserver(
			(entries) => {
				entries.forEach((entry) => {
					if (entry.isIntersecting) {
						const alert_els = node.querySelectorAll('.alert') as NodeListOf<HTMLDivElement>;
						const index = Array.prototype.indexOf.call(alert_els, entry.target);

						// console.log({ index, alert_els, entry });
						if (index !== -1) {
							idx = index;
						}
					}
				});
			},
			{
				root: node,
				threshold: 0.7
			}
		);

		// node.children.forEach((el) => observer.observe(el));
		const alert_els = Array.from(node.children) as HTMLDivElement[];
		alert_els.forEach((el) => observer.observe(el));

		return () => alert_els.forEach((el) => observer.unobserve(el));
	};

	function scroll_to_alert(i: number) {
		// console.log(i);
		if (!scroll_area) return;
		const alert_els = Array.from(scroll_area.querySelectorAll('.alert')) as HTMLDivElement[];
		alert_els[i].scrollIntoView({ behavior: 'smooth' });
	}

	function debounce_scroll_to_alert(i: number) {
		debounce(scroll_to_alert)(i);
	}

	$effect(() => {
		// remove href from all links in alert-text id. I don't want people leaving my website ):<
		scroll_area?.querySelectorAll('a').forEach((a) => {
			a.removeAttribute('href');
		});
	});
</script>

<header class="flex items-center gap-1 p-1">
	<Icon width={36} height={36} link={false} {route} />

	<div class="flex items-center gap-1 text-xl font-semibold">
		{#if route_alerts.length && idx < route_alerts.length}
			{route_alerts[idx].alert_type}
		{:else}
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
	bind:this={scroll_area}
	{@attach manage_scroll}
>
	{#each route_alerts as alert}
		<article
			class="alert flex max-h-[65dvh] w-full shrink-0 snap-start snap-always flex-col items-center justify-between gap-1"
		>
			<div class="max-h-[65dvh] overflow-auto bg-neutral-950 px-1">
				{@html alert.header_html}

				{#if alert.description_html}
					{@html alert.description_html}
				{/if}
			</div>

			{#snippet alert_time(time: Date)}
				{@const dt = dayjs(time)}
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
				{#if alert.end_time}
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
			<!-- <div class="flex  w-fit"> -->

			<button
				disabled={idx === 0}
				class:text-neutral-500={idx === 0}
				aria-label="Previous alert"
				onclick={() => scroll_to_alert(idx - 1)}
			>
				<ChevronLeft />
			</button>
			{#each route_alerts as _alert, i}
				<button
					class={['size-3 rounded-full bg-neutral-300', { 'bg-neutral-500': i === idx }]}
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
			<!-- </div> -->
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
