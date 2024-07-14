<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import type { Swiper } from 'swiper/types';
	import { alerts } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import { derived } from 'svelte/store';
	import { onMount } from 'svelte';
	dayjs.extend(relativeTime);

	export let route_id: string;

	const route_alerts = derived(alerts, ($alerts) => {
		const route_alerts = $alerts
			.filter((a) => a.entities.some((e) => e.route_id === route_id))
			.sort((a, b) => {
				return (
					b.entities.find((e) => e.route_id === route_id)!.sort_order -
					a.entities.find((e) => e.route_id === route_id)!.sort_order
				);
			});
		return route_alerts;
	});
	// console.log($route_alerts);

	function fix_swiper({ detail }: { detail: [Swiper] }) {
		const swiper = detail[0];
		setTimeout(async () => {
			console.log('fixing swiper');
			swiper.update();
			// swiper.slideReset();
		}, 500);
	}

	onMount(() => {
		// remove href from all links in alert-text id
		document
			.getElementById('alert-text')
			?.querySelectorAll('a')
			.forEach((a) => {
				a.removeAttribute('href');
			});
	});

	// TODO: show route map after alerts
</script>

<svelte:head>
	{#if $route_alerts.length}
		<title>{route_id} | {$route_alerts[0].alert_type}</title>
	{:else}
		<title>{route_id} | No alerts</title>
	{/if}
</svelte:head>

<!-- TODO: fix swiper slides breaking for certain alerts (i think i need to somehow update the component) -->
<!-- it seems to fix itself when clicking outside of the dialog (but that closes it) -->
{#if $route_alerts.length}
	<swiper-container
		style="--swiper-pagination-bullet-inactive-color: #818cf8; --swiper-pagination-color: #6366f1; background-color: #171717"
		pagination="true"
		auto-height="false"
		on:swiperinit={fix_swiper}
	>
		{#each $route_alerts as alert}
			<swiper-slide class="bg-neutral-900">
				<div class="relative flex flex-col max-h-[80dvh]">
					<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300 p-1">
						<Icon link={false} width="2rem" height="2rem" name={route_id} />
						{alert.alert_type}
					</h2>

					<div
						id="alert-text"
						class="text-indigo-200 max-h-[80dvh] overflow-auto p-1 bg-neutral-900"
					>
						<!-- hypothetically, the MTA could XSS this website (that would be silly) -->
						{@html alert.header_html}
						<!-- TODO: remove links or mark them as links -->
						{#if alert.description_html}
							{@html alert.description_html}
						{/if}
					</div>

					<div class="text-sm text-neutral-400 flex justify-between pl-1">
						<div>Updated {dayjs(alert.updated_at).fromNow()}</div>
						{#if alert.end_time}
							<!-- TODO: get the earliest end_time from API -->
							<div>End {dayjs(alert.end_time).fromNow()}</div>
						{/if}
					</div>
				</div>
			</swiper-slide>
		{/each}
	</swiper-container>
{:else}
	<div class="flex items-center gap-2 p-2">
		<Icon width="2rem" height="2rem" name={route_id} />
		<div class="text-neutral-200">No alerts</div>
	</div>
{/if}

<style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-0 bg-neutral-900;
	}
</style>
