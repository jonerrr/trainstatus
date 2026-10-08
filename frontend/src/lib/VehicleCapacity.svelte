<script lang="ts">
	import type { TypedVehiclePosition } from '#lib/resources/index.svelte.js';

	import { Users } from '@lucide/svelte';

	interface Props {
		position?: TypedVehiclePosition<'mta_bus'> | TypedVehiclePosition<'njt_bus'>;
	}

	const { position }: Props = $props();

	const occupancy = $derived.by(() => {
		if (position?.data.source === 'mta_bus') {
			if (!position.data.passengers || !position.data.capacity) return;
			const pct = Math.floor((position.data.passengers / position.data.capacity) * 100);
			return {
				kind: 'count' as const,
				label: String(position.data.passengers),
				tone: pct > 80 ? 'full' : pct > 30 ? 'busy' : 'open'
			};
		}
		if (position?.data.source !== 'njt_bus') return;
		// Five steps, shortest to tallest. Not-boarding is a slash instead of another step.
		switch (position.data.occupancy_status) {
			case 'Empty':
				return { kind: 'meter' as const, label: 'Empty', tone: 'open' as const, filled: 0 };
			case 'ManySeatsAvailable':
				return { kind: 'meter' as const, label: 'Many seats', tone: 'open' as const, filled: 1 };
			case 'FewSeatsAvailable':
				return { kind: 'meter' as const, label: 'Few seats', tone: 'busy' as const, filled: 2 };
			case 'StandingRoomOnly':
				return {
					kind: 'meter' as const,
					label: 'Standing room',
					tone: 'busy' as const,
					filled: 3
				};
			case 'CrushedStandingRoomOnly':
				return { kind: 'meter' as const, label: 'Packed', tone: 'full' as const, filled: 4 };
			case 'Full':
				return { kind: 'meter' as const, label: 'Full', tone: 'full' as const, filled: 5 };
			case 'NotAcceptingPassengers':
				return {
					kind: 'meter' as const,
					label: 'Not boarding',
					tone: 'full' as const,
					filled: 0,
					blocked: true
				};
			case 'NotBoardable':
				return {
					kind: 'meter' as const,
					label: 'Not boardable',
					tone: 'full' as const,
					filled: 0,
					blocked: true
				};
			default:
				return;
		}
	});
	const meterBars = [4, 7, 10, 13, 16];
</script>

{#if occupancy}
	<!-- TODO: better colors -->
	<div
		class={[
			'flex items-center gap-1 self-start text-neutral-200',
			{
				'text-yellow-400': occupancy.tone === 'busy',
				'text-red-400': occupancy.tone === 'full'
			}
		]}
	>
		{#if occupancy.kind === 'count'}
			<Users size="16" aria-hidden="true" />
			{occupancy.label}
		{:else}
			<svg
				viewBox="0 0 16 16"
				width="16"
				height="16"
				class="shrink-0"
				role="img"
				aria-label={occupancy.label}
			>
				<title>{occupancy.label}</title>
				{#each meterBars as height, index (index)}
					<rect
						x={index * 3.2}
						y={16 - height}
						width="2.2"
						{height}
						rx="0.4"
						fill="currentColor"
						opacity={index < occupancy.filled ? 1 : 0.28}
					/>
				{/each}
				{#if occupancy.blocked}
					<path
						d="M1.5 14.5 L14.5 1.5"
						stroke="currentColor"
						stroke-width="1.4"
						stroke-linecap="round"
					/>
				{/if}
			</svg>
		{/if}
	</div>
{/if}
