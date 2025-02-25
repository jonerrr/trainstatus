<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { ChevronRight, ChevronLeft } from 'lucide-svelte';
	import { alerts as rt_alerts } from '$lib/alerts.svelte';
	import { debounce } from '$lib/util.svelte';
	import { type Route } from '$lib/static';
	import Icon from '$lib/Icon.svelte';
	import { updated } from '$app/state';
	dayjs.extend(relativeTime);

	interface ModalProps {
		route: Route;
		time_format: 'time' | 'countdown';
	}

	let { route, time_format }: ModalProps = $props();

	const alerts = $derived(
		rt_alerts.alerts_by_route
			.get(route.id)
			?.sort(
				(a, b) =>
					b.entities.find((e) => e.route_id === route.id)!.sort_order -
					a.entities.find((e) => e.route_id === route.id)!.sort_order
			) ?? []
	);

	let idx = $state(0);

	let scroll_area: HTMLDivElement | undefined;

	function manage_scroll(node: HTMLDivElement) {
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

		return {
			destroy() {
				alert_els.forEach((el) => observer.unobserve(el));
			}
		};
	}

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

<header class="flex gap-1 p-1 items-center">
	<Icon width={36} height={36} express={false} link={false} {route} />

	<div class="text-xl font-semibold flex items-center gap-1">
		{#if alerts.length && idx < alerts.length}
			{alerts[idx].alert_type}
		{:else}
			No alerts
		{/if}
	</div>
</header>

<!-- handle arrow keys -->
<svelte:window
	onkeydown={($event) => {
		if ($event.key === 'ArrowLeft' && idx > 0) {
			// if we don't debounce, clicking arrow key twice really fast will get the scroll stuck
			debounce_scroll_to_alert(idx - 1);
		} else if ($event.key === 'ArrowRight' && idx < alerts.length - 1) {
			debounce_scroll_to_alert(idx + 1);
		}
	}}
/>

<div
	class="snap-mandatory snap-x gap-2 overflow-x-scroll flex scrollbar-hidden bg-neutral-950"
	bind:this={scroll_area}
	use:manage_scroll
>
	{#each alerts as alert}
		<article
			class="alert snap-start snap-always flex flex-col gap-1 items-center justify-between shrink-0 w-full max-h-[65dvh]"
		>
			<div class="px-1 max-h-[65dvh] overflow-auto bg-neutral-950">
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

			<div class="text-sm text-neutral-400 px-1 w-full flex justify-between">
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

	{#if alerts.length > 1}
		<div class="absolute bottom-0 -translate-y-16 w-full flex gap-2 justify-center items-center">
			<!-- <div class="flex  w-fit"> -->

			<button
				disabled={idx === 0}
				class:text-neutral-500={idx === 0}
				aria-label="Previous alert"
				onclick={() => scroll_to_alert(idx - 1)}
			>
				<ChevronLeft />
			</button>
			{#each alerts as _alert, i}
				<button
					class="rounded-full bg-neutral-300 size-3 {i !== idx && 'bg-neutral-500'}"
					aria-label="Scroll to alert"
					onclick={() => scroll_to_alert(i)}
				>
				</button>
			{/each}
			<button
				disabled={idx === alerts.length - 1}
				class:text-neutral-500={idx === alerts.length - 1}
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
