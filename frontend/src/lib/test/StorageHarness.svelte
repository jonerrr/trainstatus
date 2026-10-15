<script lang="ts">
	import { type LocalStorage } from '#lib/storage.svelte.js';

	let { value }: { value: LocalStorage<string> } = $props();
</script>

<p>{value.current}</p>
<button
	onclick={() => {
		value.current = 'changed';
	}}>Change preference</button
>
