<script lang="ts">
	import { melt, createTabs } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { derived } from 'svelte/store';
	import { stops } from '$lib/stores';
	import { Direction } from '$lib/api';
	import Trigger from '$lib/components/Trip/Trigger.svelte';
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
		defaultValue: 'northbound'
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

<div class="p-2">
	{#if $stop}
		<div class="flex items-center gap-2 py-1 max-w-[calc(100%-60px)]">
			<Routes routes={$stop.routes} />

			<h2 class="font-bold text-xl text-indigo-300">{$stop.name}</h2>
		</div>

		{#if $stop.transfers.length}
			<div class="flex gap-2 pb-1 items-center flex-wrap">
				<h2 class="text-lg">Transfers:</h2>

				{#each $stop.transfers as stop_id}
					<Transfer {stop_id} />
				{/each}
			</div>
		{/if}

		<div>
			<div
				use:melt={$root}
				class="flex border border-neutral-800 flex-col rounded-xl shadow-lg data-[orientation=vertical]:flex-row bg-neutral-900/50 text-indigo-400"
			>
				<div
					use:melt={$list}
					class="flex shrink-0 overflow-x-auto text-indigo-100
		  data-[orientation=vertical]:flex-col data-[orientation=vertical]:border-r"
					aria-label="Trip information"
				>
					{#each triggers as triggerItem}
						<button use:melt={$trigger(triggerItem.id)} class="trigger relative">
							{triggerItem.title}
							{#if $value === triggerItem.id}
								<div
									in:send={{ key: 'trigger' }}
									out:receive={{ key: 'trigger' }}
									class="absolute bottom-1 left-1/2 h-1 w-full -translate-x-1/2 rounded-full bg-indigo-400"
								/>
							{/if}
						</button>
					{/each}
				</div>
				<div use:melt={$content('northbound')} class="bg-neutral-900/50 p-2">
					<Trigger stop_id={$stop.id} direction={Direction.North} />
				</div>
				<div use:melt={$content('southbound')} class=" bg-neutral-900/50 p-2">
					<Trigger stop_id={$stop.id} direction={Direction.South} />
				</div>
			</div>
		</div>
	{:else}
		<h2>Invalid stop ID</h2>
	{/if}
</div>

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
		}
	}
</style>
