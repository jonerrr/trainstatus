<!-- TODO: connect to map route -->
<div class="absolute z-50 top-0 left-0">
	<div class="flex flex-col gap-1 p-2 m-2 rounded-lg bg-white/50 dark:bg-black/50 backdrop-blur-md">
		<!-- <div class="text-lg font-bold">Realtime Bus Map</div> -->

		<div class="flex justify-between gap-1 items-center min-w-52">
			<div class="text-lg font-semibold">Filters</div>
			<button
				onclick={() => {
					filters_open = !filters_open;
				}}
				class="underline text-sm text-blue-500 hover:text-blue-700"
				>{filters_open ? 'Hide' : 'Show'}</button
			>
		</div>
		{#if filters_open}
			<div class="flex flex-col gap-1" transition:slide>
				<label class="grid grid-cols-[1fr_auto] items-center gap-2">
					<span>Routes</span>
					<input bind:checked={show_routes} type="checkbox" />
				</label>
				<label class="grid grid-cols-[1fr_auto] items-center gap-2">
					<span>Stops</span>
					<input bind:checked={show_stops} type="checkbox" />
				</label>
				<label class="grid grid-cols-[1fr_auto] items-center gap-2">
					<span>Trips</span>
					<input bind:checked={show_trips} type="checkbox" />
				</label>
				{#if show_trips}
					<div class="flex flex-col gap-1" transition:slide>
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span>Overlapping</span>
							<input bind:checked={show_overlapping} type="checkbox" />
						</label>
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span>Unknown passengers</span>
							<input bind:checked={show_unknown} type="checkbox" />
						</label>
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span>Min passengers</span>
							<input
								bind:value={min_val}
								type="number"
								inputmode="numeric"
								min="0"
								class="w-16 rounded-sm border border-neutral-900 dark:border-neutral-100"
							/>
						</label>
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span>Max passengers</span>
							<input
								bind:value={max_val}
								type="number"
								inputmode="numeric"
								min="0"
								class="w-16 rounded-sm border border-neutral-900 dark:border-neutral-100"
							/>
						</label>
					</div>
				{/if}
			</div>
		{/if}
		<div class="text-xs mt-2">
			Data from <a
				href="https://trainstat.us"
				target="_blank"
				rel="noopener noreferrer"
				class="underline text-blue-500 hover:text-blue-700">Train Status</a
			>
		</div>
	</div>
</div>
