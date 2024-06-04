<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { type Alert, type RouteAlerts, route_alerts_store } from '$lib/api';
	import { pinned_routes } from '$lib/stores';
	import { Dialog } from '$lib/components/Dialog';
	import Icon from '$lib/components/Icon.svelte';
	import Pin from '$lib/components/Pin.svelte';
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

<Dialog.Trigger name={route_id}>
	<div class="flex gap-2 items-center">
		<Icon width="2rem" height="2rem" name={route_id} />

		{#if route_alerts && route_alerts.alerts.length}
			<div class="font-semibold flex gap-2 items-center">
				<div>
					{route_alerts.alerts[0].alert_type}
				</div>
			</div>
			<!-- TODO: stop +n from moving on page load -->
			{#if route_alerts.alerts.length > 1}
				<div class="font-normal rounded bg-indigo-200 p-1 text-neutral-800">
					+{route_alerts.alerts.length - 1}
				</div>
			{/if}
		{:else}
			<div class="text-neutral-400">No alerts</div>
		{/if}
	</div>

	<div>
		<Pin item_id={route_id} store={pinned_routes} />
	</div>
</Dialog.Trigger>

<Dialog.Content name={route_id} let:title let:description let:close>
	{#if route_alerts}
		<swiper-container
			pagination="true"
			auto-height="true"
			class="max-h-[85vh]"
			style="--swiper-pagination-bullet-inactive-color: #0a0a0a; --swiper-pagination-color: #6366f1;"
			on:swiperprogress={on_progress}
		>
			{#each route_alerts.alerts as alert}
				<swiper-slide class="relative">
					<h2
						class="sticky top-0 font-bold flex items-center gap-2 text-indigo-300 bg-neutral-800"
						use:melt={title}
					>
						<Icon width="2rem" height="2rem" name={route_id} />
						{alert.alert_type}
					</h2>

					<div
						use:melt={description}
						class="text-indigo-200 overflow-auto max-h-[calc(85vh-8rem)] border border-neutral-700 rounded mb-6 mt-2"
					>
						{@html alert.header}
						{#if alert.description}
							{@html alert.description}
						{/if}
					</div>

					<div class="text-sm text-neutral-400">
						<div>Updated {dayjs(alert.updated_at).fromNow()}</div>
						<!-- TODO: show end if not undefined -->
						<!-- <div>Updated {dayjs(alert.updated_at).fromNow()}</div> -->
					</div>
					<span class="text-sm text-neutral-400"> </span>
				</swiper-slide>
			{/each}
		</swiper-container>
	{:else}
		<div class="text-indigo-200">No alerts</div>
	{/if}
	<button
		class="z-40 text-indigo-400 font-bold absolute bottom-0 right-0 rounded p-2 m-6 shadow-xl bg-neutral-900/75 active:bg-neutral-800 hover:bg-neutral-800"
		use:melt={close}>Close</button
	>
</Dialog.Content>

<style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-2;
	}
</style>
