<script lang="ts">
	import type { TypedVehiclePosition } from '$lib/resources/index.svelte';

	import { Users } from '@lucide/svelte';

	interface Props {
		// vehicle_id: string;
		// // only supports sources that have position data, which currently is just mta_bus but we can add more later if needed
		// source: 'mta_bus';

		position?: TypedVehiclePosition<'mta_bus'> | TypedVehiclePosition<'njt_bus'>;
	}

	const { position }: Props = $props();

	// const positions = $derived(position_context.getSource(source));

	// const position = $derived(positions.value?.get(vehicle_id));

	const pct_full = $derived.by(() => {
		switch (position?.data.source) {
			case 'mta_bus':
				if (!position.data.passengers || !position.data.capacity) break;
				return Math.floor((position.data.passengers / position.data.capacity) * 100);
			case 'njt_bus':
				if (!position.data.occupancy_status) break;
				// this is really rough and just an estimate based on the descriptions of each occupancy status, but it's better than nothing
				switch (position.data.occupancy_status) {
					case 'Empty':
					case 'NoDataAvailable':
						return 0;
					case 'ManySeatsAvailable':
						return 20;
					case 'FewSeatsAvailable':
						return 50;
					case 'StandingRoomOnly':
						return 80;
					case 'CrushedStandingRoomOnly':
						return 90;
					case 'Full':
					case 'NotAcceptingPassengers':
					case 'NotBoardable':
						return 100;
				}
		}
		return undefined;
	});
</script>

{#if position && pct_full}
	<!-- TODO: better colors -->
	<div
		class={[
			'flex items-center gap-1 self-start text-neutral-200',
			{
				'text-yellow-400': pct_full > 30,
				'text-red-400': pct_full > 80
			}
		]}
	>
		<Users size="16" />
		<!-- currently only mta_bus returns passenger count -->
		{#if 'passengers' in position?.data}
			{position.data.passengers}
		{/if}
	</div>
{/if}
