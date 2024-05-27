<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	// import emblaCarouselSvelte from 'embla-carousel-svelte';
	// import AutoHeight from 'embla-carousel-auto-height';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import type { Alert, RouteAlerts } from '$lib/api';
	import { pinned_routes } from '$lib/stores';
	import { Dialog } from '$lib/components/Dialog';
	import Icon from '$lib/components/Icon.svelte';
	import Pin from '$lib/components/Pin.svelte';
	dayjs.extend(relativeTime);

	export let route_id: string;
	export let alerts: RouteAlerts | undefined;
	// TODO: worry about sort_order

	// let plugins = [AutoHeight()];
	// const options = { loop: true };
</script>

<Dialog.Trigger name={route_id}>
	<div class="flex gap-2 items-center">
		<Icon width="2rem" height="2rem" name={route_id} />

		{#if alerts}
			<div class="font-semibold flex gap-2">
				<div>
					{alerts.alerts[0].alert_type}
				</div>
				{#if alerts.alerts.length > 1}
					<div class="font-normal rounded bg-indigo-200 p-1 text-neutral-800">
						<!-- {alerts.alerts.length > 1 ? `+${alerts.alerts.length - 1}` : ''} -->
						+{alerts.alerts.length - 1}
					</div>
				{/if}
			</div>
		{:else}
			<div class="text-neutral-400">No alerts</div>
		{/if}
	</div>

	<div>
		<Pin item_id={route_id} store={pinned_routes} />
	</div>
</Dialog.Trigger>

<Dialog.Content name={route_id} let:title let:description let:close>
	{#if alerts}
		<swiper-container
			pagination="true"
			auto-height="true"
			style="--swiper-pagination-bullet-inactive-color: #171717; --swiper-pagination-color: #6366f1;"
		>
			{#each alerts?.alerts as alert}
				<swiper-slide>
					<h2 class="font-bold flex items-center gap-2 text-indigo-300" use:melt={title}>
						<Icon width="2rem" height="2rem" name={route_id} />
						{alert.alert_type}
					</h2>

					<div use:melt={description} class="text-indigo-200">
						{@html alert.header}
						{#if alert.description}
							{@html alert.description}
						{/if}

						<span class="text-sm text-neutral-400">
							Updated {dayjs(alert.updated_at).fromNow()}
						</span>
					</div>
				</swiper-slide>
			{/each}
		</swiper-container>
		<div class="flex text-indigo-200">
			<button class="btn mt-2 ml-auto" use:melt={close}>Close</button>
		</div>
	{:else}
		<div class="text-indigo-200">No alerts</div>
	{/if}
</Dialog.Content>

<style lang="postcss">
</style>
