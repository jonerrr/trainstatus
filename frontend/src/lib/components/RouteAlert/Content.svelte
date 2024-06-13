<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import type { Swiper } from 'swiper/types';
	import { alerts } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import { derived } from 'svelte/store';
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
	console.log($route_alerts);

	function fix_swiper({ detail }: { detail: [Swiper] }) {
		console.log('fixing swiper');

		const swiper = detail[0];
		setTimeout(async () => {
			console.log('fixing swiper');
			swiper.update();
			// swiper.slideReset();
		}, 500);
	}
</script>

<!-- TODO: fix swiper slides breaking for certain alerts (i think i need to somehow update the component) -->
<!-- it seems to fix itself when clicking outside of the dialog (but that closes it) -->
<!-- <div class="max-h-[80vh]"> -->
{#if $route_alerts.length}
	<swiper-container
		style="--swiper-pagination-bullet-inactive-color: #0a0a0a; --swiper-pagination-color: #6366f1;"
		pagination="true"
		auto-height="false"
		observer="true"
		observe-parents="true"
		loop={$route_alerts.length > 1}
		on:swiperinit={fix_swiper}
	>
		{#each $route_alerts as alert}
			<swiper-slide>
				<div class="relative flex flex-col max-h-[80vh]">
					<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300">
						<Icon width="2rem" height="2rem" name={route_id} />
						{alert.alert_type}
					</h2>

					<div class="text-indigo-200 overflow-auto border border-neutral-700 rounded my-2">
						<!-- hypothetically, the MTA could XSS this website (that would be silly) -->
						{@html alert.header_html}
						<!-- TODO: remove links or mark them as links -->
						{#if alert.description_html}
							{@html alert.description_html}
						{/if}
					</div>

					<div class="text-sm text-neutral-400 flex justify-between">
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
	<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300">
		<Icon width="2rem" height="2rem" name={route_id} />
	</h2>
	<div class="text-indigo-200">No alerts</div>
{/if}

<!-- </div> -->

<style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-2;
	}

	/* :global(.swiper-slide) {
		width: fit-content;
	} */
</style>
