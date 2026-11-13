<script lang="ts" generics="P extends object">
	import type { Component } from 'svelte';

	import { setTransitData, type TransitData } from '#lib/data/store.svelte.js';
	import DataBoundary from '#lib/DataBoundary.svelte';
	import { createCurrentTime, setCurrentTime } from '#lib/url-params.svelte.js';

	let {
		data,
		component,
		childProps
	}: { data: TransitData; component: Component<P>; childProps?: P } = $props();

	const View = $derived(component);

	// svelte-ignore state_referenced_locally
	setTransitData(data);
	setCurrentTime(createCurrentTime());
</script>

<DataBoundary><View {...childProps as P} /></DataBoundary>
