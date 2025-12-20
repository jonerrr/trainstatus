<script lang="ts">
	import { fade } from 'svelte/transition';

	import { pushState } from '$app/navigation';

	import { CloudOff, Settings } from '@lucide/svelte';

	interface Props {
		offline: boolean;
	}

	let { offline }: Props = $props();
</script>

<header class="relative flex justify-between overflow-x-auto bg-neutral-900 p-2 text-sm text-white">
	<div class="flex items-center gap-1">
		<div class="gradient-text text-3xl font-semibold tracking-tight text-nowrap md:text-4xl">
			Train Status
		</div>
		{#if offline}
			<div transition:fade class="flex flex-col items-center text-red-500">
				<CloudOff class="size-6" />
				<div class="self-end">Offline</div>
			</div>
		{/if}
	</div>
	<div class="flex items-center justify-center gap-2">
		<button
			aria-label="Open settings"
			title="Settings"
			class="flex items-center justify-center rounded-md border border-neutral-700/50 bg-neutral-800/70 p-2 text-neutral-300 transition-all duration-200 hover:bg-neutral-700 hover:text-blue-400 focus:ring-2 focus:ring-blue-500/30 focus:outline-none active:bg-neutral-600"
			onclick={() => {
				pushState('', { modal: 'settings' });
			}}
		>
			<Settings class="size-5" />
		</button>
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
