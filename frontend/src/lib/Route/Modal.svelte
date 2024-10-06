<script lang="ts">
	import { page } from '$app/stores';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import { type Route, type Stop, type TrainStopData } from '$lib/static';
	import { persisted_rune } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';

	interface ModalProps {
		route: Route;
	}

	let { route }: ModalProps = $props();
</script>

<div class="flex gap-1 pb-1 items-center">
	<!-- {#if large} -->
	<Icon width="1.5rem" height="1.5rem" express={false} link={false} {route} />

	<div class="font-medium text-lg">
		{route.long_name}
	</div>
</div>
