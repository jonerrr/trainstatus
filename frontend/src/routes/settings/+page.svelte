<script lang="ts">
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';

	import { all_sources, source_info } from '#lib/source_info.js';
	import { source_preferences } from '#lib/source_preferences.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import {
		BookText,
		CircleX,
		CodeXml,
		ExternalLink,
		Hourglass,
		Info,
		Layers
	} from '@lucide/svelte';
	import dayjs from 'dayjs';

	const current_time = getCurrentTime();
	let selected_time = $derived(
		current_time.value === undefined
			? ''
			: dayjs.unix(current_time.value).format('YYYY-MM-DDTHH:mm')
	);

	function applyTime(at: number | undefined) {
		const url = new URL(page.url.href);
		if (at === undefined) url.searchParams.delete('at');
		else url.searchParams.set('at', String(at));
		void goto(url.pathname + url.search + url.hash, { state: page.state });
	}

	// TODO: improve ui
	// TODO: add 'debug' mode that logs data in console and shows extra info about data sources
</script>

<div class="h-full overflow-y-auto bg-black text-white">
	<div class="mx-auto max-w-4xl px-6 py-6">
		<!-- <h1 class="mb-12 text-4xl font-bold">Settings</h1> -->

		<div class="flex flex-col divide-y divide-neutral-800 text-base">
			<div class="p-6">
				<div class="mb-4 flex items-center justify-between">
					<div class="flex items-center gap-1">
						<Layers class="size-5" />
						<h2 class="text-xl font-semibold">Data Sources</h2>
					</div>
				</div>
				<p class="mb-3 pl-6 text-sm text-neutral-400">
					Select which transit agencies to load data from. Enabling more sources may increase data
					usage and load times.
				</p>

				<div class="flex flex-col gap-3 pl-6">
					{#each all_sources as source (source)}
						{const info = $derived(source_info[source])}
						{const enabled = $derived(source_preferences.current.includes(source))}
						<label
							class="flex cursor-pointer items-center gap-3 rounded-md py-2 transition-colors hover:text-emerald-400"
						>
							<input
								type="checkbox"
								checked={enabled}
								disabled={source_preferences.current.length === 1 && enabled}
								onchange={() => {
									source_preferences.toggle(source);
									refreshAll();
								}}
								class="size-5 rounded border-neutral-700 bg-neutral-800 text-emerald-500 focus:ring-emerald-500/30 disabled:opacity-50"
							/>
							<div class="flex items-center gap-2">
								<enhanced:img
									src={info.icon}
									sizes="24px"
									alt=""
									class="size-6 rounded-sm object-contain"
								/>
								<span class="font-medium">{info.name}</span>
							</div>
						</label>
					{/each}
				</div>
			</div>

			<div class="p-6">
				<div class="mb-4 flex items-center justify-between">
					<div class="flex items-center gap-1">
						<Hourglass class="size-5" />
						<h2 class="text-xl font-semibold">Time Travel</h2>
						<span class="text-xs text-yellow-300">(Beta)</span>
					</div>
				</div>
				<p class="mb-3 pl-6 text-sm text-neutral-400">
					View train and bus information from a specific time in the past. This allows you to see
					historical train movements and delays for research or analysis purposes.
				</p>

				<form
					class="flex flex-wrap items-center gap-2 pl-6"
					onsubmit={(event) => {
						event.preventDefault();
						applyTime(dayjs(selected_time).unix());
					}}
				>
					<input
						max={dayjs().format('YYYY-MM-DDTHH:mm')}
						style="color-scheme: dark; font-size: 0.875rem"
						type="datetime-local"
						aria-label="Historical time"
						required
						bind:value={selected_time}
						class="min-w-50 rounded border border-neutral-700 bg-transparent p-2 leading-6 text-neutral-400"
					/>
					<button
						type="submit"
						disabled={!selected_time}
						class="rounded bg-emerald-700 px-4 py-2 hover:bg-emerald-600 disabled:opacity-50"
						>Apply</button
					>
					{#if current_time.value !== undefined}
						<button
							type="button"
							title="Return to live data"
							aria-label="Return to live data"
							onclick={() => {
								selected_time = '';
								applyTime(undefined);
							}}
							class="transition-colors duration-300 hover:text-fuchsia-300"
						>
							<CircleX class="size-6" />
						</button>
					{/if}
				</form>
				{#if current_time.value !== undefined}
					<p class="mt-2 text-xs text-fuchsia-400">
						Currently viewing data from: {dayjs
							.unix(current_time.value)
							.format('MMMM D, YYYY h:mm A')}
					</p>
				{/if}
			</div>

			<!-- TODO: move resources to its own page -->
			<div class="p-6">
				<div class="mb-4 flex items-center justify-between">
					<div class="flex items-center gap-1">
						<Info class="size-5" />
						<h3 class="text-xl font-semibold">Resources</h3>
					</div>
				</div>

				<div class="flex flex-col gap-2">
					<a
						href="/api/docs"
						target="_blank"
						class="flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-blue-400 focus:ring-2 focus:ring-blue-500/30 focus:outline-none active:scale-98"
					>
						<div>
							<div class="flex items-center gap-1">
								<BookText class="size-5" />
								<span>Api Documentation</span>
								<ExternalLink class="size-4" />
							</div>
							<div class="text-sm text-neutral-400">
								Access Train Status data for your own projects
							</div>
						</div>
					</a>

					<a
						href="https://github.com/jonerrr/trainstatus"
						target="_blank"
						class="flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-green-400 focus:ring-2 focus:ring-green-500/30 focus:outline-none active:scale-98"
					>
						<div>
							<div class="flex items-center gap-1">
								<CodeXml class="size-5" />
								<span>Source Code</span>
								<ExternalLink class="size-4" />
							</div>
							<div class="text-sm text-neutral-400">View and contribute on GitHub</div>
						</div>
					</a>
				</div>
			</div>
		</div>
	</div>
</div>
