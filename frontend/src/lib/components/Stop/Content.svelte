<script lang="ts">
	import { melt, createTabs } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { stops } from '$lib/stores';
	import { Direction, StopType } from '$lib/api';
	import { stop_times } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Trigger from '$lib/components/Trip/Trigger.svelte';

	export let stop_id: string;

	const stop = $stops.find((s) => s.id === stop_id)!;

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'northbound'
	});

	const triggers = [
		{ id: 'northbound', title: stop.north_headsign },
		{ id: 'southbound', title: stop.south_headsign }
	];

	const [send, receive] = crossfade({
		duration: 250,
		easing: cubicInOut
	});

	// TODO: show transfers
</script>

<div class="flex items-center gap-2 py-1">
	<!-- TODO: differentiate between fulltime and part time routes and temporary routes for icon -->
	<div class="flex gap-1">
		{#each stop.routes as route (route.id)}
			<Icon
				class={route.stop_type === StopType.FullTime || route.stop_type === StopType.PartTime
					? ''
					: 'opacity-30'}
				width="2rem"
				height="2rem"
				name={route.id}
			/>
		{/each}
	</div>

	<h2 class="font-bold text-xl text-indigo-300">{stop.name}</h2>
</div>

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
			<Trigger stop_id={stop.id} direction={Direction.North} />
		</div>
		<div use:melt={$content('southbound')} class=" bg-neutral-900/50 p-2">
			<Trigger stop_id={stop.id} direction={Direction.South} />
		</div>
	</div>
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
