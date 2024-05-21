<script lang="ts">
	import dayjs from 'dayjs';
	// import updateLocale from 'dayjs/plugin/updateLocale.js';
	// import relativeTime from 'dayjs/plugin/relativeTime.js';
	import type { Stop, StopTime } from '$lib/api';

	// dayjs.extend(updateLocale);
	// dayjs.updateLocale('en', {
	// 	relativeTime: {
	// 		future: '%s',
	// 		past: '%s ago',
	// 		s: 'soon',
	// 		m: 'm',
	// 		mm: '%dm',
	// 		h: 'an hour',
	// 		hh: '%d hours',
	// 		d: 'a day',
	// 		dd: '%d days',
	// 		M: 'a month',
	// 		MM: '%d months',
	// 		y: 'a year',
	// 		yy: '%d years'
	// 	}
	// });
	// dayjs.extend(relativeTime);

	export let stop_time: StopTime | undefined;
	// console.log(stop_time?.arrival);
	const arrival = new Date(stop_time!.arrival);
	console.log(arrival);
	$: eta = stop_time ? Math.floor((arrival.getTime() - Date.now()) / (1000 * 60)) : undefined;

	// $: eta = stop_time ? dayjs(stop_time.arrival).fromNow() : undefined;
	// $: eta = stop_time ? arrival.set - Date.now() / 1000 / 60 : undefined;
	// let eta = 1;
	// console.log(eta);
</script>

<div class="text-xs">
	{#if eta}
		{eta.toFixed(0)}m
	{:else}
		Arriving
	{/if}
</div>
