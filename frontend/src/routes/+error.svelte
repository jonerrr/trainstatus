<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	import ErrorView from '#lib/ErrorView.svelte';

	import type { ErrorProps } from './$types';

	let { error }: ErrorProps = $props();
	function retry() {
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- Retry the exact shared URL without adding history.
		return goto(page.url.href, { replace: true, refreshAll: true, state: page.state });
	}
</script>

<ErrorView {error} onretry={retry} />
