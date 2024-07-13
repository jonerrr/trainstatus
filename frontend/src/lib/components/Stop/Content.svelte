<script lang="ts">
	import { melt, createTabs } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { derived } from 'svelte/store';
	import { stops, stop_direction } from '$lib/stores';
	import { Direction } from '$lib/api';
	import TripList from '$lib/components/Trip/StopTimeList.svelte';
	import Routes from '$lib/components/Stop/Routes.svelte';
	import Transfer from '$lib/components/Stop/Transfer.svelte';

	export let stop_id: string;

	const stop = derived(stops, ($stops) => {
		return $stops.find((s) => s.id === stop_id);
	});

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'northbound',
		value: stop_direction
	});

	const triggers = [
		{ id: 'northbound', title: $stop?.north_headsign },
		{ id: 'southbound', title: $stop?.south_headsign }
	];

	const [send, receive] = crossfade({
		duration: 250,
		easing: cubicInOut
	});
</script>

<svelte:head>
	{#if $stop}
		<title>{$stop.routes.map((r) => r.id).join(', ')} | {$stop.name}</title>
	{/if}
</svelte:head>

{#if $stop}
	<div class="flex items-center gap-2 p-1">
		<h2 class="font-bold text-xl text-indigo-300">{$stop.name}</h2>
		<Routes link={true} routes={$stop.routes} />
	</div>

	{#if $stop.transfers.length}
		<div class="flex gap-2 pb-1 px-1 items-center flex-wrap">
			<h2 class="text-base text-indigo-200">Transfers:</h2>

			{#each $stop.transfers as stop_id}
				<Transfer {stop_id} />
			{/each}
		</div>
	{/if}

	<div use:melt={$root} class="flex flex-col shadow-2xl text-indigo-400">
		<div use:melt={$content('northbound')}>
			<TripList stop={$stop} direction={Direction.North} />
		</div>
		<div use:melt={$content('southbound')}>
			<TripList stop={$stop} direction={Direction.South} />
		</div>
		<div
			use:melt={$list}
			class="flex shrink-0 overflow-x-auto text-indigo-100 bg-neutral-800"
			aria-label="Trip information"
		>
			{#each triggers as triggerItem}
				<button use:melt={$trigger(triggerItem.id)} class="trigger relative">
					{triggerItem.title}
					{#if $value === triggerItem.id}
						<div
							in:send={{ key: 'trigger' }}
							out:receive={{ key: 'trigger' }}
							class="absolute top-1 left-1/2 h-1 w-1/2 -translate-x-1/2 rounded-full bg-indigo-400"
						/>
					{/if}
				</button>
			{/each}
		</div>
	</div>
{/if}

<style lang="postcss">
	.trigger {
		display: flex;
		align-items: center;
		justify-content: center;

		cursor: default;
		user-select: none;

		border-radius: 0;

		font-weight: 500;
		line-height: 1;

		flex: 1;
		height: theme(spacing.12);
		padding-inline: theme(spacing.2);

		&:focus {
			position: relative;
		}

		&:focus-visible {
			@apply z-10 ring-2;
		}

		&[data-state='active'] {
			@apply focus:relative;
			color: theme('colors.indigo.200');
			background-color: theme('colors.neutral.900');
		}
	}
</style>
