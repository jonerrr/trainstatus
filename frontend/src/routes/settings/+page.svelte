<script lang="ts">
	import { current_time } from '$lib/url_params.svelte';

	import { BookText, CodeXml, ExternalLink, Hourglass, Info, Map } from '@lucide/svelte';
	import { CircleX } from '@lucide/svelte';
	import dayjs from 'dayjs';
</script>

<div class="min-h-screen bg-black text-white">
	<div class="mx-auto max-w-4xl px-6 py-12">
		<h1 class="mb-12 text-4xl font-bold">Settings</h1>

		<div class="flex flex-col divide-y divide-neutral-800 text-base">
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

				<div class="flex items-center gap-2 pl-6">
					<input
						max={dayjs().format('YYYY-MM-DDTHH:mm')}
						style="color-scheme: dark; font-size: 0.875rem"
						type="datetime-local"
						bind:value={
							() =>
								current_time.value ? dayjs.unix(current_time.value).format('YYYY-MM-DDTHH:mm') : '',
							(v) => (current_time.value = dayjs(v).unix())
						}
						class="min-w-50 rounded border border-neutral-700 bg-transparent p-2 leading-6 text-neutral-400"
					/>
					{#if current_time.value}
						<button
							title="Clear time"
							onclick={() => {
								current_time.value = undefined;
							}}
							class="transition-colors duration-300 hover:text-fuchsia-300"
						>
							<CircleX class="size-6" />
						</button>
					{/if}
				</div>
				{#if current_time.value}
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
					<!-- <a
					href={charts_href}
					class="active:scale-98 flex items-center gap-2 rounded-md p-2 pl-6 transition-all duration-200 hover:bg-neutral-800/50 hover:text-emerald-400 focus:outline-none focus:ring-2 focus:ring-emerald-500/30 active:bg-neutral-800"
				>
					<div>
						<div class="flex items-center gap-1">
							<ChartLine class="size-5" />
							<span>Charts</span>
						</div>
						<div class="text-xs text-neutral-400">Subway and Bus string lines</div>
					</div>
				</a> -->

					<a
						href="https://map.trainstat.us"
						target="_blank"
						class="active:scale-98 flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-rose-400 focus:outline-none focus:ring-2 focus:ring-rose-500/30"
					>
						<div>
							<div class="flex items-center gap-1">
								<Map class="size-5" />
								<span>Bus Map</span>
								<ExternalLink class="size-4" />
							</div>
							<div class="text-sm text-neutral-400">A realtime map of buses in New York City</div>
						</div>
					</a>

					<a
						href="/api/docs"
						target="_blank"
						class="active:scale-98 flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-blue-400 focus:outline-none focus:ring-2 focus:ring-blue-500/30"
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
						class="active:scale-98 flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-green-400 focus:outline-none focus:ring-2 focus:ring-green-500/30"
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
