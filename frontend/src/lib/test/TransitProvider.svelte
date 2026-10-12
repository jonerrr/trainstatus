<script lang="ts" generics="P extends Record<string, unknown>">
	import type { Component } from 'svelte';

	import { setTransitData, type TransitData } from '#lib/transit/store.svelte.js';
	import { createCurrentTime, setCurrentTime } from '#lib/url_params.svelte.js';

	let {
		data,
		component: Child,
		child_props
	}: { data: TransitData; component: Component<P>; child_props?: P } = $props();

	// svelte-ignore state_referenced_locally
	setTransitData(data);
	setCurrentTime(createCurrentTime());
</script>

<Child {...child_props as P} />
