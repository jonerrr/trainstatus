<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import emblaCarouselSvelte from 'embla-carousel-svelte';
	import AutoHeight from 'embla-carousel-auto-height';
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

	let plugins = [AutoHeight()];
</script>

<Dialog.Trigger name={route_id}>
	<div class="flex gap-2 items-center">
		<Icon width="2rem" height="2rem" name={route_id} />

		{#if alerts}
			<div class="font-semibold flex gap-2">
				<div>
					{alerts.alerts[0].alert_type}
				</div>
				<div class="font-normal">
					{alerts.alerts.length > 1 ? `+${alerts.alerts.length - 1}` : ''}
				</div>
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
		<div class="embla" use:emblaCarouselSvelte={{ plugins }}>
			<div class="embla__container">
				{#each alerts?.alerts as alert}
					<div class="embla__slide">
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
					</div>
				{/each}
			</div>
		</div>
		<div class="flex text-indigo-200">
			<button class="btn mt-2 ml-auto" use:melt={close}>Close</button>
		</div>
	{:else}
		<div class="text-indigo-200">No alerts</div>
	{/if}
</Dialog.Content>

<style>
	.embla {
		overflow: hidden;
	}
	.embla__container {
		display: flex;
		align-items: flex-start;
		transition: height 0.2s;
	}
	.embla__slide {
		flex: 0 0 100%;
		min-width: 0;
	}
</style>
