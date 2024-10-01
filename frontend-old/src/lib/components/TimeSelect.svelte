<script lang="ts">
	// TODO:  put this in layout
	import { page } from '$app/stores';
	import { data_at } from '$lib/stores';
	import { onMount } from 'svelte';

	onMount(() => {
		let time = $page.url.searchParams.get('at');

		if (time) {
			console.log('setting time');
			data_at.set(new Date(parseInt(time) * 1000));
		}
	});
</script>

{#if $data_at}
	<time datetime={$data_at.toISOString()} class="text-xs text-neutral-400">
		{$data_at.toLocaleString()}
	</time>
{/if}
