<script lang="ts">
	import { untrack } from 'svelte';

	import { createCurrentTime, setCurrentTime } from '../url-params.svelte';
	import CurrentTimeValue from './CurrentTimeValue.svelte';

	const { at }: { at?: number } = $props();
	setCurrentTime(createCurrentTime(untrack(() => at)));
</script>

<CurrentTimeValue />
