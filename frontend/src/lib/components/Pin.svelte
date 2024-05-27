<script lang="ts">
	import { createToggle, melt, createSync } from '@melt-ui/svelte';
	import type { Writable } from 'svelte/store';
	import { Pin } from 'lucide-svelte';
	import { addToast } from '$lib/components/UndoToaster.svelte';

	export let item_id: string;
	export let store: Writable<string[]>;

	const {
		elements: { root },
		states
	} = createToggle();
	const sync = createSync(states);
	$: sync.pressed($store.includes(item_id), (v) => {
		if (v) {
			$store = [...$store, item_id];
		} else {
			$store = $store.filter((id) => id !== item_id);
			// TODO: fix undo toaster
			// addToast({ title: 'stop removed ', description: 'removed', item: 'Stop', item_id });
		}
	});
</script>

<button
	use:melt={$root}
	on:click|stopPropagation
	aria-label="Pin stop to home screen"
	class="z-50 grid h-9 w-9 place-items-center items-center justify-center rounded-md
bg-white text-base leading-4 text-fuchsia-800 shadow-lg hover:bg-indigo-200
data-[disabled]:cursor-not-allowed data-[state=on]:bg-indigo-400
"
>
	<Pin />
</button>
