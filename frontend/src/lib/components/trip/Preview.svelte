<!-- TODO: add share button -->
<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import type { Alert, RouteAlerts, Trip } from '$lib/api';
	import { stops } from '$lib/stores';
	import { Dialog } from '$lib/components/Dialog';
	import Icon from '$lib/components/Icon.svelte';
	// import Pin from '$lib/components/Pin.svelte';
	dayjs.extend(relativeTime);

	export let trip: Trip;
</script>

<Dialog.Trigger name={trip.id}>
	<div
		class="w-full border-neutral-700 bg-neutral-800 rounded border shadow-2xl hover:bg-neutral-900 px-1 text-neutral-300"
	>
		<!-- TODO: show current stop / how many stops away -->
		<div class="flex gap-2 items-center justify-between mx-1">
			<div class="flex gap-2 items-center">
				<div class=""><Icon name={trip.route_id} /></div>
				<div>
					{trip.eta?.toFixed(0)}m
				</div>
			</div>
			<div class="text-right">
				{$stops.find((s) => s.id === trip.stop_times[trip.stop_times.length - 1].stop_id)?.name}
			</div>
		</div>
	</div>
</Dialog.Trigger>

<Dialog.Content name={trip.id} let:title let:description let:close>
	<div class="flex items-center gap-2 py-1" use:melt={title}>
		<Icon name={trip.route_id} /> title
	</div>

	<div use:melt={description}>description</div>
	<button
		class="z-40 text-indigo-400 font-bold absolute bottom-0 right-0 rounded p-2 m-6 shadow-xl bg-neutral-900/75 active:bg-neutral-800 hover:bg-neutral-800"
		use:melt={close}>Close</button
	>
</Dialog.Content>

<!-- <style lang="postcss">
	swiper-container::part(pagination) {
		@apply sticky bottom-2;
	}
</style> -->
