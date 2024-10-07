<script lang="ts" generics="T extends string | number">
	import type { Snippet } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import type { PersistedRune } from './util.svelte';
	import Pin from './Pin.svelte';
	// import { Pin } from 'lucide-svelte';

	// export let state: App.PageState;
	let {
		state,
		id,
		pin_rune = $bindable(),
		children
	}: {
		state: App.PageState;
		id?: string;
		pin_rune?: PersistedRune<T[]>;
		children: Snippet;
	} = $props();
</script>

<div {id} class="relative w-full list-item">
	<button
		class="w-full flex justify-between items-center py-1 hover:bg-neutral-900 active:bg-neutral-900"
		transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
		onclick={() => {
			pushState('', JSON.parse(JSON.stringify(state)));
		}}
	>
		{@render children()}
	</button>

	{#if pin_rune}
		<Pin
			bind:pin_rune
			id={state.data.id}
			class="absolute z-20 right-0 py-1 px-2 rounded-md text-neutral-300 hover:text-neutral-400 top-[50%] transform -translate-y-1/2"
		/>
		<!-- <button
			onclick={() => {
				// console.log('pin button', pin_rune.value, id, pin_rune.value.includes(id));
				pin_rune.value = pin_rune.value.includes(state.data.id)
					? pin_rune.value.filter((item) => item !== state.data.id)
					: [...pin_rune.value, state.data.id];
			}}
			aria-label="Pin to home screen"
			class="absolute z-20 right-0 py-1 px-2 rounded-md text-neutral-300 hover:text-neutral-400 top-[50%] transform -translate-y-1/2"
		>
			{#if pin_rune.value.includes(state.data.id)}
				<Pin fill="#d4d4d4" />
			{:else}
				<Pin />
			{/if}
		</button> -->
	{/if}
</div>
