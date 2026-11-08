<script lang="ts">
	import type { FilterFieldDef, FilterValue } from './filters.svelte';

	interface Props {
		fieldDef: FilterFieldDef;
		value?: FilterValue;
		onChange: (value: FilterValue) => void;
	}

	let { fieldDef, value, onChange }: Props = $props();

	function handleBooleanChange(checked: boolean) {
		// An unchecked checkbox removes the filter.
		const newValue = checked ? true : undefined;
		onChange(newValue);
	}

	function handleEnumChange(selectedValue: string) {
		const selected = Array.isArray(value) ? value : [];
		const next = selected.includes(selectedValue)
			? selected.filter((option) => option !== selectedValue)
			: [...selected, selectedValue];
		onChange(next.length ? next : undefined);
	}

	function handleStringChange(inputValue: string) {
		onChange(inputValue.length > 0 ? inputValue : undefined);
	}
</script>

<div class="flex flex-col gap-1.5">
	{#if fieldDef.type === 'boolean'}
		<label class="grid min-h-10 grid-cols-[1fr_auto] items-center gap-2 rounded-none px-1">
			<span class="text-sm">{fieldDef.label}</span>
			<input
				type="checkbox"
				checked={value === true}
				onchange={(e) => handleBooleanChange(e.currentTarget.checked)}
				class="size-5 cursor-pointer"
			/>
		</label>
	{:else if fieldDef.type === 'enum'}
		<div class="flex flex-col gap-1">
			<span class="text-sm">{fieldDef.label}</span>
			<!-- Multi-select: checkboxes -->
			<div class="ml-1.5 flex flex-col gap-0.5">
				{const stringValues = $derived(Array.isArray(value) ? value : [])}
				{#each fieldDef.options as option (option)}
					{const isChecked = $derived(stringValues.includes(option))}
					<label class="grid min-h-10 grid-cols-[1fr_auto] items-center gap-2 rounded-none px-1">
						<span class="text-xs">{option}</span>
						<input
							type="checkbox"
							checked={isChecked}
							onchange={() => handleEnumChange(option)}
							class="size-5 cursor-pointer"
						/>
					</label>
				{/each}
			</div>
		</div>
	{:else if fieldDef.type === 'string'}
		<label class="flex flex-col gap-1">
			<span class="text-sm">{fieldDef.label}</span>
			<input
				type="text"
				value={typeof value === 'string' ? value : ''}
				onchange={(e) => handleStringChange(e.currentTarget.value)}
				placeholder="Search..."
				class="min-h-10 rounded-none border border-border-strong bg-surface-2 px-2 py-1 text-sm text-primary"
			/>
		</label>
	{/if}
</div>
