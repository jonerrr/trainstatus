<script lang="ts">
	import { melt, createTabs, name } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { Dialog } from '$lib/components/Dialog';
	import { pinned_stops } from '$lib/stores';
	import { Direction, type Stop, type StopType } from '$lib/api';
	import Pin from '$lib/components/Pin.svelte';
	import Arrivals from '$lib/components/Stop/Arrivals.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Trips from '$lib/components/Stop/Trips.svelte';

	export let stop: Stop;

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'northbound'
	});

	// TODO: replace title with more specific destination (maybe last stop boroughs or headsign)
	const triggers = [
		{ id: 'northbound', title: stop.north_headsign },
		{ id: 'southbound', title: stop.south_headsign }
	];

	const [send, receive] = crossfade({
		duration: 250,
		easing: cubicInOut
	});
</script>

<Dialog.Trigger name={'stop/' + stop.id}>
	<div class="w-[25%] grow-0 font-semibold text-indigo-200">
		{stop.name}
	</div>

	<div class="flex flex-col w-[30%] mt-auto">
		<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
			{stop.north_headsign}
		</div>
		<!-- TODO: go through stop times and take missing routes from there to show  -->
		<div class="flex flex-col gap-1">
			{#each stop.routes as route (route.id)}
				<Arrivals {route} stop_id={stop.id} direction={Direction.North} />
			{/each}
		</div>
	</div>

	<div class="flex flex-col w-[30%] mt-auto">
		<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
			{stop.south_headsign}
		</div>
		<div class="flex flex-col gap-1">
			{#each stop.routes as route (route.id)}
				<Arrivals {route} stop_id={stop.id} direction={Direction.South} />
			{/each}
		</div>
	</div>

	<div>
		<Pin item_id={stop.id} store={pinned_stops} />
	</div>
</Dialog.Trigger>

<Dialog.Content name={'stop/' + stop.id} let:title let:description let:close>
	<div class="justify-center">
		<div class="flex items-center gap-2 py-1" use:melt={title}>
			<!-- TODO: only show normal stopping trains or somehow indicate that route doesn't stop there all times -->
			<!-- TODO: make icons adjust in size and wrap if 4+ routes -->
			<div class="flex gap-1">
				{#each stop.routes as route (route.id)}
					<Icon width="2rem" height="2rem" name={route.id} />
				{/each}
			</div>

			<h2 class="font-bold text-xl text-indigo-300">{stop.name}</h2>
		</div>

		<div use:melt={description}>
			<div
				use:melt={$root}
				class="flex max-w-[25rem] border border-neutral-800 flex-col overflow-hidden rounded-xl shadow-lg data-[orientation=vertical]:flex-row bg-neutral-900/50 text-indigo-400 mb-12"
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
				<div use:melt={$content('northbound')} class="grow bg-neutral-900/50 p-2">
					<Trips stop_id={stop.id} direction={Direction.North} />
				</div>
				<div use:melt={$content('southbound')} class="grow bg-neutral-900/50 p-2">
					<Trips stop_id={stop.id} direction={Direction.South} />
				</div>
			</div>

			<!-- <div class="fixed bottom-0 flex px-3 w-full">
				<button
					class="z-40 text-indigo-400 mx-auto font-bold rounded shadow-xl bg-neutral-900/75 mt-2 h-12 active:bg-neutral-800 hover:bg-neutral-800"
					use:melt={close}
					>Close
				</button>
			</div> -->
			<button
				class="z-40 ml-1 text-indigo-400 font-bold absolute bottom-0 right-0 rounded p-2 m-6 shadow-xl bg-neutral-900/75 active:bg-neutral-800 hover:bg-neutral-800"
				use:melt={close}>Close</button
			>
		</div>
	</div>
</Dialog.Content>

<style lang="postcss">
	.trigger {
		display: flex;
		align-items: center;
		justify-content: center;

		cursor: default;
		user-select: none;

		border-radius: 0;

		/* color: theme(colors.neutral.900); */
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
