<script lang="ts">
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';

	import { sourceIcons } from '#lib/source-icons.js';
	import { sourcePreferences } from '#lib/source-preferences.svelte.js';
	import { allSources, sourceConfig } from '#lib/sources.js';
	import { getCurrentTime } from '#lib/url-params.svelte.js';

	import { BookText, CodeXml, ExternalLink, Hourglass, Info, Layers } from '@lucide/svelte';
	import dayjs from 'dayjs';

	const currentTime = getCurrentTime();
	const appliedTime = $derived(
		currentTime.value === undefined ? '' : dayjs.unix(currentTime.value).format('YYYY-MM-DDTHH:mm')
	);
	let selectedTime = $derived(appliedTime);
	const timeDirty = $derived(selectedTime !== appliedTime);
	let applyingTime = $state(false);

	async function applyTime(at: number | undefined) {
		if (applyingTime) return;
		const url = new URL(page.url.href);
		if (at === undefined) url.searchParams.delete('at');
		else url.searchParams.set('at', String(at));
		applyingTime = true;
		try {
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- The current browser URL already includes the base path.
			await goto(url.pathname + url.search + url.hash, { state: page.state });
		} finally {
			applyingTime = false;
		}
	}

	// TODO: improve ui
	// TODO: add 'debug' mode that logs data in console and shows extra info about data sources
</script>

<div class="h-full overflow-y-auto bg-canvas text-white">
	<div class="mx-auto max-w-4xl px-6 py-6">
		<!-- <h1 class="mb-12 text-4xl font-bold">Settings</h1> -->

		<div class="flex flex-col divide-y divide-border-subtle text-base">
			<div class="p-6">
				<div class="mb-4 flex items-center justify-between">
					<div class="flex items-center gap-1">
						<Layers class="size-5" />
						<h2 class="text-xl font-semibold">Data Sources</h2>
					</div>
				</div>
				<p class="mb-3 pl-6 text-sm text-muted">
					Select which transit agencies to load data from. Enabling more sources may increase data
					usage and load times.
				</p>

				<div class="flex flex-col gap-3 pl-6">
					{#each allSources as source (source)}
						{const info = $derived(sourceConfig[source])}
						{const enabled = $derived(sourcePreferences.current.includes(source))}
						<label
							class="flex cursor-pointer items-center gap-3 rounded-none py-2 transition-colors hover:text-accent"
						>
							<input
								type="checkbox"
								checked={enabled}
								disabled={sourcePreferences.current.length === 1 && enabled}
								onchange={() => {
									sourcePreferences.toggle(source);
									refreshAll();
								}}
								class="size-5 rounded-none border-border-default bg-surface-2 text-accent checked:border-accent checked:bg-accent focus:ring-accent/30 disabled:opacity-50"
							/>
							<div class="flex items-center gap-2">
								<enhanced:img
									src={sourceIcons[source]}
									sizes="24px"
									alt=""
									class="size-6 rounded-none object-contain"
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
					</div>
				</div>
				<p class="mb-3 pl-6 text-sm text-muted">
					View train and bus information from a specific time in the past. This allows you to see
					historical train movements and delays for research or analysis purposes.
				</p>

				<form
					class="flex flex-wrap items-center gap-2 pl-6"
					onsubmit={(event) => {
						event.preventDefault();
						if (selectedTime && timeDirty) void applyTime(dayjs(selectedTime).unix());
					}}
				>
					<input
						max={dayjs().format('YYYY-MM-DDTHH:mm')}
						style:color-scheme="dark"
						style:font-size="0.875rem"
						type="datetime-local"
						aria-label="Historical time"
						required
						disabled={applyingTime}
						bind:value={selectedTime}
						class="min-w-50 rounded-none border border-border-default bg-transparent p-2 leading-6 text-muted"
					/>
					<button
						type="submit"
						disabled={!selectedTime || !timeDirty || applyingTime}
						class="rounded-none bg-accent px-4 py-2 text-accent-fg transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent enabled:hover:bg-accent-hover disabled:opacity-50"
						>{applyingTime ? 'Applying…' : appliedTime && !timeDirty ? 'Applied' : 'Apply'}</button
					>
					{#if currentTime.value !== undefined}
						<button
							type="button"
							disabled={applyingTime}
							onclick={() => void applyTime(undefined)}
							class="rounded-none border border-border-default px-4 py-2 text-secondary transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent enabled:hover:border-neutral-500 enabled:hover:bg-hover enabled:hover:text-white disabled:opacity-50"
						>
							Return to live
						</button>
					{/if}
				</form>
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
						rel="external"
						target="_blank"
						class="flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-accent focus:ring-2 focus:ring-accent/30 focus:outline-none active:scale-98"
					>
						<div>
							<div class="flex items-center gap-1">
								<BookText class="size-5" />
								<span>Api Documentation</span>
								<ExternalLink class="size-4" />
							</div>
							<div class="text-sm text-muted">Access Train Status data for your own projects</div>
						</div>
					</a>

					<a
						href="https://github.com/jonerrr/trainstatus"
						target="_blank"
						class="flex items-center gap-3 px-4 py-3 transition-all duration-200 hover:text-accent focus:ring-2 focus:ring-accent/30 focus:outline-none active:scale-98"
					>
						<div>
							<div class="flex items-center gap-1">
								<CodeXml class="size-5" />
								<span>Source Code</span>
								<ExternalLink class="size-4" />
							</div>
							<div class="text-sm text-muted">View and contribute on GitHub</div>
						</div>
					</a>
				</div>
			</div>
		</div>
	</div>
</div>
