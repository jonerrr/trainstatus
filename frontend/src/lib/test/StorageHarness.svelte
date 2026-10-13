<script lang="ts">
	import { onMount } from 'svelte';

	import { syncLocalStorage, type LocalStorage } from '#lib/storage.svelte.js';

	let { value }: { value: LocalStorage<string> } = $props();
	onMount(syncLocalStorage);
</script>

<p>{value.current}</p>
<button
	onclick={() => {
		value.current = 'changed';
	}}>Change preference</button
>
