<script lang="ts">
	import { getCurrentTime } from '../url-params.svelte';

	const time = getCurrentTime();
</script>

<output>{time.value === undefined ? 'live' : `historical:${time.value}:${time.ms}`}</output>
