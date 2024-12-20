<script lang="ts">
	import dayjs from 'dayjs';
	import { BookText, GitBranch, CloudOff } from 'lucide-svelte';
	import { fade } from 'svelte/transition';
	import { current_time } from '$lib/util.svelte';

	interface Props {
		offline: boolean;
	}

	let { offline }: Props = $props();
</script>

<header class="text-4xl p-2 font-bold flex justify-between relative bg-neutral-900">
	<div class="flex gap-1">
		<div class="gradient-text font-black">Train Status</div>
		{#if offline}
			<div transition:fade class="text-red-500 flex flex-col items-center">
				<CloudOff class="w-6 h-6" />
				<div class=" text-xs self-end">Offline</div>
			</div>
		{/if}
		{#if current_time.value}
			<!-- <div
				class="text-neutral-400 text-xs flex flex-col justify-center items-center hover:underline"
			>
				<span>{time.format('DD/MM')}</span>
				<span>{time.format('h:m A')}</span>
			</div> -->
			<!-- TODO: update rt data after value change  -->
			<!-- TODO: show input even if user didn't specify in query param -->
			<!-- TODO: update url param with user's input -->
			<input
				type="datetime-local"
				bind:value={() =>
					current_time.value ? dayjs.unix(current_time.value).format('YYYY-MM-DDTHH:mm') : '',
				(v) => (current_time.value = dayjs(v).unix())}
				class="text-neutral-400 text-sm bg-transparent border-b border-neutral-400"
			/>
		{/if}
	</div>
	<!-- <TimeSelect /> -->
	<div class="flex justify-center items-center gap-2">
		<a
			href="/api/docs"
			target="_blank"
			class="text-white text-sm hover:text-blue-400 transition-colors duration-300 flex flex-col items-center"
		>
			<BookText class="w-6 h-6" />
			<span>API</span>
		</a>
		<a
			href="https://github.com/jonerrr/trainstatus"
			target="_blank"
			class="text-white text-sm hover:text-green-400 transition-colors duration-300 flex flex-col items-center"
		>
			<GitBranch class="w-6 h-6" />
			<span>Code</span>
		</a>
	</div>
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
		background: linear-gradient(-45deg, #03045e, #0077b6, #00b4d8, #e73c7e, #23d5ab);
		background-size: 400% 400%;
		animation: gradient 15s ease infinite;
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}
</style>
