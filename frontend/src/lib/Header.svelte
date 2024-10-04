<script lang="ts">
	// import { offline } from '$lib/stores';
	// import TimeSelect from '$lib/components/TimeSelect.svelte';
</script>

<header class="text-4xl p-2 font-bold flex justify-between relative">
	<div class="gradient-text font-bold">Trainstat.us</div>

	<!-- <TimeSelect /> -->

	<!-- {#if $offline}
        <div class="text-red-500 text-xs">Offline</div>
    {/if} -->
</header>

<style>
	@keyframes wave {
		0% {
			transform: translateX(0) translateY(0);
		}
		25% {
			transform: translateX(-25%) translateY(5%);
		}
		50% {
			transform: translateX(-50%) translateY(-5%);
		}
		75% {
			transform: translateX(-75%) translateY(5%);
		}
		100% {
			transform: translateX(-100%) translateY(0);
		}
	}

	@keyframes gradient {
		0% {
			background-position: 0% 50%;
		}
		25% {
			background-position: 50% 75%;
		}
		50% {
			background-position: 100% 50%;
		}
		75% {
			background-position: 50% 25%;
		}
		100% {
			background-position: 0% 50%;
		}
	}

	.gradient-text {
		/* background: linear-gradient(-45deg, #ee7752, #e73c7e, #23a6d5, #23d5ab); */
		background: linear-gradient(-45deg, #03045e, #0077b6, #00b4d8, #e73c7e, #23d5ab);
		background-size: 400% 400%;
		animation: gradient 15s ease infinite;
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}
</style>
