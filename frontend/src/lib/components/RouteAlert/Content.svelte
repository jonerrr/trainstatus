<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { route_alerts_store } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	dayjs.extend(relativeTime);

	export let route_id: string;
	// export let route_alerts: RouteAlerts | undefined;
	// TODO: worry about sort_order

	function on_progress(e: any) {
		const [swiper, progress] = e.detail;
		const index = swiper.activeIndex;
		// console.log(index);
	}

	$: route_alerts = $route_alerts_store.find((ra) => ra.route_id === route_id);
</script>

{#if route_alerts}
	<swiper-container
		pagination="true"
		auto-height="true"
		class="max-w-96"
		style="--swiper-pagination-bullet-inactive-color: #0a0a0a; --swiper-pagination-color: #6366f1;"
		on:swiperprogress={on_progress}
	>
		{#each route_alerts.alerts as alert}
			<swiper-slide>
				<h2 class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300 bg-neutral-800">
					<Icon width="2rem" height="2rem" name={route_id} />
					{alert.alert_type}
				</h2>

				<div
					class="text-indigo-200 overflow-auto min-w-72 max-h-96 border border-neutral-700 rounded mb-6 mt-2"
				>
					{@html alert.header}
					{#if alert.description}
						{@html alert.description}
					{/if}
				</div>

				<div class="text-sm text-neutral-400 pr-12">
					<div>Updated {dayjs(alert.updated_at).fromNow()}</div>
					<!-- TODO: show end if not undefined -->
					<!-- <div>Updated {dayjs(alert.updated_at).fromNow()}</div> -->
				</div>
				<!-- <span class="text-sm text-neutral-400"> </span> -->
			</swiper-slide>
		{/each}
	</swiper-container>
{:else}
	<!-- TODO: show route_id on top and links to stuff even when no alert -->
	<div class="text-indigo-200 h-20">No alerts</div>
{/if}

<style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-2;
	}
</style>
