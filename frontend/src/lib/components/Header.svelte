<script lang="ts">
	import { createSwitch, melt } from '@melt-ui/svelte';
	import { persisted } from 'svelte-persisted-store';
	import { offline, bus_mode } from '$lib/stores';

	const {
		elements: { root, input }
	} = createSwitch({ checked: bus_mode });
</script>

<header class="text-4xl p-2 font-bold text-indigo-400 flex justify-between">
	<div
		class="font-bold"
		style="background: linear-gradient(45deg, #e66465, #9198e5); -webkit-background-clip: text; -webkit-text-fill-color: transparent;"
	>
		{#if $offline}
			Trainstat<span class="animate-pulse text-red-500">.</span>us
		{:else}
			Trainstat.us
		{/if}
	</div>
	<div class="flex items-center">
		<label
			class="pr-2 leading-none text-indigo-600 font-semibold text-sm"
			for="airplane-mode"
			id="airplane-mode-label"
		>
			Bus mode
		</label>
		<button
			use:melt={$root}
			class="relative h-6 cursor-default rounded-full bg-neutral-800 transition-colors data-[state=checked]:bg-indigo-700"
			id="airplane-mode"
			aria-labelledby="airplane-mode-label"
		>
			<span class="thumb block rounded-full bg-white transition" />
		</button>
		<input use:melt={$input} />
	</div>
</header>

<style lang="postcss">
	.text-gradient {
		/* background-image: linear-gradient(45deg, #f3ec78, #af4261);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent; */
		--bg-size: 400%;
		--color-one: #4338ca;
		--color-two: #a21caf;
		font-family: sans-serif;
		background: linear-gradient(90deg, var(--color-one), var(--color-two), var(--color-one)) 0 0 /
			var(--bg-size) 100%;
		color: transparent;
		background-clip: text;
	}

	@media (prefers-reduced-motion: no-preference) {
		.text-gradient {
			animation: move-bg 8s linear infinite;
		}
		@keyframes move-bg {
			to {
				background-position: var(--bg-size) 0;
			}
		}
	}

	/* switch css */
	button {
		--w: 2.75rem;
		--padding: 0.125rem;
		width: var(--w);
	}

	.thumb {
		--size: 1.25rem;
		width: var(--size);
		height: var(--size);
		transform: translateX(var(--padding));
	}

	:global([data-state='checked']) .thumb {
		transform: translateX(calc(var(--w) - var(--size) - var(--padding)));
	}
</style>
