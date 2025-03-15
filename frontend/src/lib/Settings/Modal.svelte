<script lang="ts">
	import dayjs from 'dayjs';
	import { BookText, GitBranch, Hourglass, CircleX, Settings, Map } from 'lucide-svelte';
	import { current_time } from '$lib/util.svelte';
	import { replaceState } from '$app/navigation';
	import { tick } from 'svelte';

	$effect(() => {
		current_time.value;
		tick().then(() => {
			const url = new URL(window.location.href);

			// use existing url because we don't want to lose other query params
			if (current_time.value) {
				url.searchParams.set('at', current_time.value.toString());
			} else {
				url.searchParams.delete('at');
			}

			// only update url if it has changed
			const new_url = url.toString();
			if (new_url !== window.location.href) {
				replaceState(new_url, {
					modal: 'settings'
				});
			}
		});
	});
</script>

<div class="flex gap-1 items-center p-3 z-20">
	<Settings />
	<div class="text-xl text-white">Settings</div>
</div>

<div
	class="flex flex-col max-h-[60dvh] border-y bg-neutral-950 border-neutral-800 divide-y overflow-auto divide-neutral-800 text-base"
>
	<!-- Time Settings -->
	<div class="p-4">
		<div class="flex items-center justify-between mb-3">
			<div class="flex items-center gap-1">
				<Hourglass class="size-5" />
				<h3 class="text-lg font-medium">Custom Time</h3>
				<h1 class="text-xs text-yellow-300">(Beta)</h1>
			</div>
		</div>
		<p class="text-sm text-neutral-400 mb-3">
			View train and bus information from a specific time in the past. This allows you to see
			historical train movements and delays for research or analysis purposes.
		</p>

		<div class="flex gap-2 items-center">
			<input
				max={dayjs().format('YYYY-MM-DDTHH:mm')}
				style="color-scheme: dark; font-size: 0.875rem"
				type="datetime-local"
				tabindex={-1}
				bind:value={
					() =>
						current_time.value ? dayjs.unix(current_time.value).format('YYYY-MM-DDTHH:mm') : '',
					(v) => (current_time.value = dayjs(v).unix())
				}
				class="text-neutral-400 bg-transparent border rounded border-neutral-700 p-2 leading-6 min-w-[200px]"
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
		{#if current_time.value}
			<p class="text-xs text-fuchsia-400 mt-2">
				Currently viewing data from: {dayjs.unix(current_time.value).format('MMMM D, YYYY h:mm A')}
			</p>
		{/if}
	</div>

	<!-- Links -->
	{#snippet link(title: string, description: string, url: string, color: string)}
		<a
			href={url}
			target="_blank"
			class="flex items-center gap-2 p-2 rounded-md transition-all duration-200 hover:bg-neutral-800/50 hover:text-{color}-400 active:bg-neutral-800 active:scale-98 focus:outline-none focus:ring-2 focus:ring-{color}-500/30"
		>
			<BookText class="size-5" />
			<div>
				<div class="flex items-center gap-1">
					<span>{title}</span>
					<span class="text-xs opacity-60">↗</span>
				</div>
				<div class="text-xs text-neutral-400">{description}</div>
			</div>
		</a>
	{/snippet}

	<div class="p-4">
		<h3 class="text-lg font-medium mb-3">Resources</h3>
		<div class="flex flex-col gap-4">
			{@render link(
				'API Documentation',
				'Access Train Status data for your own projects',
				'/api/docs',
				'blue'
			)}
			{@render link(
				'Source Code',
				'View and contribute on GitHub',
				'https://github.com/jonerrr/trainstatus',
				'green'
			)}
		</div>
	</div>
</div>
