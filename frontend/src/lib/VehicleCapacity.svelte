<script lang="ts">
	import { Users } from '@lucide/svelte';

	import type { TypedVehiclePosition } from './sources/index.svelte';

	interface Props {
		// vehicle_id: string;
		// // only supports sources that have position data, which currently is just mta_bus but we can add more later if needed
		// source: 'mta_bus';

		position?: TypedVehiclePosition<'mta_bus'>;
	}

	const { position }: Props = $props();

	// const positions = $derived(position_context.getSource(source));

	// const position = $derived(positions.value?.get(vehicle_id));
</script>

{#if position?.data.passengers && position?.data.capacity}
	{@const pct_full = Math.floor((position.data.passengers / position.data.capacity) * 100)}
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
		{position.data.passengers}
	</div>
{/if}
