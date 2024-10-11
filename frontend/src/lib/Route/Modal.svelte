<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { ChevronRight, ChevronLeft } from 'lucide-svelte';
	// import { page } from '$app/stores';
	import { alerts as rt_alerts } from '$lib/alerts.svelte';
	import { type Route } from '$lib/static';
	import Icon from '$lib/Icon.svelte';
	dayjs.extend(relativeTime);

	interface ModalProps {
		route: Route;
	}

	let { route }: ModalProps = $props();

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

	// $effect(() => {
	// 	// check if idx is out of bounds
	// 	if (idx >= alerts.length) {
	// 		idx = 0
	// 	}
	// });

	let scroll_area: HTMLDivElement | undefined;

	function manage_scroll(node: HTMLDivElement) {
		const observer = new IntersectionObserver(
			(entries) => {
				entries.forEach((entry) => {
					if (entry.isIntersecting) {
						const alert_els = node.querySelectorAll('.alert') as NodeListOf<HTMLDivElement>;
						const index = Array.prototype.indexOf.call(alert_els, entry.target);

						console.log({ index, alert_els, entry });
						if (index !== -1) {
							idx = index;
						}
					}
				});
			},
			{
				root: node,
				threshold: 1.0
			}
		);

		// alert_els.forEach((el) => observer.observe(el));
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

	// $inspect(alerts);

	$effect(() => {
		// onMount(() => {
		// remove href from all links in alert-text id. I don't want people leaving my website ):<
		scroll_area?.querySelectorAll('a').forEach((a) => {
			a.removeAttribute('href');
		});
		// });

		// console.log({ idx });
	});
</script>

<div class="flex gap-1 p-1 items-center">
	<Icon width="2rem" height="2rem" express={false} link={false} {route} />

	<div class="font-medium text-lg flex items-center gap-1">
		{#if alerts.length && idx < alerts.length}
			{alerts[idx].alert_type}
		{:else}
			No alerts
		{/if}

		{#if alerts.length > 1}
			<div class="bg-neutral-700 text-neutral-50 rounded p-1">
				+{alerts.length - 1}
			</div>
		{/if}
	</div>
</div>

<div
	class="snap-mandatory snap-x px-1 overflow-x-scroll flex scrollbar-hidden"
	bind:this={scroll_area}
	use:manage_scroll
>
	{#if alerts.length > 1}
		<button
			disabled={idx === 0}
			class:text-neutral-500={idx === 0}
			class="absolute left-0 inset-y-0"
			aria-label="Previous alert"
			onclick={() => scroll_to_alert(idx - 1)}
		>
			<ChevronLeft />
		</button>
	{/if}
	{#if alerts.length > 1}
		<button
			disabled={idx === alerts.length - 1}
			class:text-neutral-500={idx === alerts.length - 1}
			class="absolute right-0 inset-y-0"
			aria-label="Next alert"
			onclick={() => scroll_to_alert(idx + 1)}
		>
			<ChevronRight />
		</button>
	{/if}
	{#each alerts as alert}
		<div
			class="alert relative snap-start snap-always flex flex-col gap-1 items-center justify-center shrink-0 w-full max-h-[65dvh]"
		>
			<div
				class={`${alerts.length > 1 ? 'w-10/12' : ''} max-h-[65dvh] overflow-auto bg-neutral-950`}
			>
				{@html alert.header_html}

				{#if alert.description_html}
					{@html alert.description_html}
				{/if}
			</div>
			<div class="text-s text-neutral-400 px-1 w-full flex justify-between">
				<div>Updated {dayjs(alert.updated_at).fromNow()}</div>
				{#if alert.end_time}
					<div>End {dayjs(alert.end_time).fromNow()}</div>
				{/if}
			</div>
		</div>
	{/each}
</div>
