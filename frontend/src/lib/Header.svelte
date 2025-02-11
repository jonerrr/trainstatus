<script lang="ts">
	import dayjs from 'dayjs';
	import { BookText, GitBranch, CloudOff, Hourglass, CircleX } from 'lucide-svelte';
	import { fade, slide } from 'svelte/transition';
	import { current_time } from '$lib/util.svelte';
	import { replaceState } from '$app/navigation';
	import { tick } from 'svelte';

	interface Props {
		offline: boolean;
	}

	let { offline }: Props = $props();
	let last_at = current_time.value;

	$effect(() => {
		current_time.value;
		tick().then(() => {
			if (!current_time.value || last_at === current_time.value) return;
			// use existing url because we don't want to lose other query params
			const url = new URL(window.location.href);
			url.searchParams.set('at', current_time.value.toString());
			// Users can't change the time if they are in a modal, so it will always be null (hopefully).
			replaceState(url.toString(), {
				modal: null
			});
		});
	});

	let show_input = $state(!!current_time.value);

	// TODO: fix min width calculation
	let always_visible_width = $state(0);
	let links_width = $state(0);
	let show_links_min_width = $derived(always_visible_width + links_width + 10);
</script>

<header
	class="p-2 text-sm flex text-white justify-between relative bg-neutral-900 overflow-x-scroll"
>
	<div class="flex gap-1 items-center" bind:offsetWidth={always_visible_width}>
		<div class="gradient-text text-nowrap text-3xl md:text-4xl font-semibold tracking-tight">
			Train Status
		</div>
		<button
			title="Change time"
			onclick={() => (show_input = !show_input)}
			class="hover:text-fuchsia-300 transition-colors duration-300 flex flex-col items-center {show_input &&
				'text-fuchsia-400'}"
		>
			<Hourglass class="size-6" />
			<span>Time</span>
		</button>
		{#if show_input}
			<div transition:slide={{ axis: 'x' }} class="flex gap-1 items-center">
				<input
					max={dayjs().format('YYYY-MM-DDTHH:mm')}
					style="color-scheme: dark; font-size: 0.75rem"
					type="datetime-local"
					bind:value={
						() =>
							current_time.value ? dayjs.unix(current_time.value).format('YYYY-MM-DDTHH:mm') : '',
						(v) => (current_time.value = dayjs(v).unix())
					}
					class="text-neutral-400 bg-transparent border-b border-neutral-400 p-0 leading-6"
				/>
				{#if current_time.value}
					<button
						title="Clear time"
						onclick={() => (current_time.value = undefined)}
						class="hover:text-fuchsia-300 transition-colors duration-300"
					>
						<CircleX class="size-6" />
					</button>
				{/if}
			</div>
		{/if}
		{#if offline}
			<div transition:fade class="text-red-500 flex flex-col items-center">
				<CloudOff class="size-6" />
				<div class="self-end">Offline</div>
			</div>
		{/if}
	</div>
	<div
		bind:offsetWidth={links_width}
		class="justify-center items-center gap-2 flex {show_input ? 'hidden sm:flex' : ''}"
	>
		<a
			href="/api/docs"
			target="_blank"
			class=" hover:text-blue-400 transition-colors duration-300 flex flex-col items-center"
		>
			<BookText class="size-6" />
			<span>API</span>
		</a>
		<a
			href="https://github.com/jonerrr/trainstatus"
			target="_blank"
			class="hover:text-green-400 transition-colors duration-300 flex flex-col items-center"
		>
			<GitBranch class="size-6" />
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
