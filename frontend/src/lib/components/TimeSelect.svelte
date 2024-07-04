<script lang="ts">
	import { page } from '$app/stores';
	import { current_time } from '$lib/stores';
	import { onMount } from 'svelte';

	onMount(() => {
		let time = $page.url.searchParams.get('at');

		if (time) {
			console.log('setting time');
			current_time.set(new Date(parseInt(time) * 1000));
		}
	});
</script>
