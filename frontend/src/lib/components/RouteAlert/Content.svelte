<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { alerts } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import { derived } from 'svelte/store';
	dayjs.extend(relativeTime);

	export let route_id: string;

	function on_progress(e: any) {
		const [swiper, progress] = e.detail;
		const index = swiper.activeIndex;
		// console.log(index);
	}

	const route_alerts = derived(alerts, ($alerts) => {
		const route_alerts = $alerts
			.filter((a) => a.entities.some((e) => e.route_id === route_id))
			.sort((a, b) => {
				return (
					a.entities.find((e) => e.route_id === route_id)!.sort_order -
					b.entities.find((e) => e.route_id === route_id)!.sort_order
				);
			});
		return route_alerts;
	});
</script>

<!-- TODO: fix swiper slides breaking for certain alerts -->
{#if $route_alerts.length}
	<swiper-container
		pagination="true"
		auto-height="true"
		loop="true"
		style="--swiper-pagination-bullet-inactive-color: #0a0a0a; --swiper-pagination-color: #6366f1;"
		on:swiperprogress={on_progress}
	>
		{#each $route_alerts as alert}
			<swiper-slide>
				<div class="">
					<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300">
						<Icon width="2rem" height="2rem" name={route_id} />
						{alert.alert_type}
					</h2>

					<div class="text-indigo-200 overflow-auto border border-neutral-700 rounded mb-6 mt-2">
						{@html alert.header_html}
						<!-- TODO: remove links or mark them as links -->
						{#if alert.description_html}
							{@html alert.description_html}
						{/if}
					</div>

					<div class="text-sm text-neutral-400 pr-12">
						<div>Updated {dayjs(alert.updated_at).fromNow()}</div>
						{#if alert.end_time}
							<div>End {dayjs(alert.end_time).fromNow()}</div>
						{/if}
					</div>
				</div>
			</swiper-slide>
		{/each}
	</swiper-container>
{:else}
	<!-- TODO: show route_id on top and links to stuff even when no alert -->
	<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300">
		<Icon width="2rem" height="2rem" name={route_id} />
	</h2>
	<div class="text-indigo-200">No alerts</div>
{/if}

<style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-2;
	}

	/* :global(.swiper-slide) {
		width: fit-content;
	} */
</style>
