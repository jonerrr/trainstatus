<script lang="ts">
	import { createSwitch, melt } from '@melt-ui/svelte';
	import { type Writable } from 'svelte/store';

	export let checked: Writable<boolean>;

	const {
		elements: { root, input }
	} = createSwitch({ checked });
</script>

<form>
	<div class="flex items-center">
		<label class="pr-4 leading-none text-neutral-300" for="past-stops" id="past-stops-label">
			Past stops
		</label>
		<button
			use:melt={$root}
			class="relative h-6 cursor-default rounded-full bg-neutral-900 transition-colors data-[state=checked]:bg-indigo-900"
			id="past-stops"
			aria-labelledby="past-stops-label"
		>
			<span class="thumb block rounded-full bg-white transition" />
		</button>
		<input use:melt={$input} />
	</div>
</form>

<style>
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
