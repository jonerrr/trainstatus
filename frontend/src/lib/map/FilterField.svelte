<script lang="ts">
	import type { FilterFieldDef, FilterValue } from './filters.svelte';

	interface Props {
		label: string;
		fieldDef: FilterFieldDef;
		value?: FilterValue;
		onChange?: (value: FilterValue) => void;
	}

	let { label, fieldDef, value = undefined, onChange }: Props = $props();

	function handleBooleanChange(checked: boolean) {
		// undefined = "all" (no filter), false = explicitly unchecked, true = checked
		const newValue = checked ? true : undefined;
		onChange?.(newValue);
	}

	function handleEnumChange(selectedValue: string, multiSelect: boolean) {
		if (!multiSelect) {
			onChange?.(selectedValue);
			return;
		}

		// Multi-select: toggle in array
		const currentArray =
			Array.isArray(value) && value.length === 2 && typeof value[0] === 'number'
				? []
				: Array.isArray(value)
					? value
					: [];
		const stringArray = currentArray.filter((v): v is string => typeof v === 'string');

		if (stringArray.includes(selectedValue)) {
			const newValue = stringArray.filter((v) => v !== selectedValue);
			onChange?.(newValue.length > 0 ? newValue : undefined);
		} else {
			onChange?.([...stringArray, selectedValue] as FilterValue);
		}
	}

	function handleStringChange(inputValue: string) {
		onChange?.(inputValue.length > 0 ? inputValue : undefined);
	}

	// Determine if we should use multi-select for enum (when there are multiple options)
	const isMultiSelect = $derived(fieldDef.type === 'enum' && (fieldDef.options?.length ?? 0) > 2);
</script>

<div class="flex flex-col gap-2">
	<!-- TODO: add "undefined" option in checkbox -->
	{#if fieldDef.type === 'boolean'}
		<label class="grid grid-cols-[1fr_auto] items-center gap-2">
			<span class="text-sm">{label}</span>
			<input
				type="checkbox"
				checked={value === true}
				onchange={(e) => handleBooleanChange(e.currentTarget.checked)}
				class="cursor-pointer"
			/>
		</label>
	{:else if fieldDef.type === 'enum'}
		<div class="flex flex-col gap-1">
			<span class="text-sm">{label}</span>
			{#if isMultiSelect}
				<!-- Multi-select: checkboxes -->
				<div class="flex flex-col gap-1 ml-2">
					{#each fieldDef.options ?? [] as option (option)}
						{@const stringValues =
							Array.isArray(value) && value.length > 0 && typeof value[0] === 'string'
								? (value as string[])
								: []}
						{@const isChecked = stringValues.includes(option)}
						<label class="grid grid-cols-[1fr_auto] items-center gap-2">
							<span class="text-xs">{option}</span>
							<input
								type="checkbox"
								checked={isChecked}
								onchange={() => handleEnumChange(option, true)}
								class="cursor-pointer"
							/>
						</label>
					{/each}
				</div>
			{:else}
				<!-- Single-select: dropdown -->
				<select
					value={typeof value === 'string' ? value : ''}
					onchange={(e) => handleEnumChange(e.currentTarget.value, false)}
					class="text-sm rounded border border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-800 px-2 py-1"
				>
					<option value="">All</option>
					{#each fieldDef.options ?? [] as option (option)}
						<option value={option}>{option}</option>
					{/each}
				</select>
			{/if}
		</div>
	{:else if fieldDef.type === 'string'}
		<label class="flex flex-col gap-1">
			<span class="text-sm">{label}</span>
			<input
				type="text"
				value={typeof value === 'string' ? value : ''}
				onchange={(e) => handleStringChange(e.currentTarget.value)}
				placeholder="Search..."
				class="text-sm rounded border border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-800 px-2 py-1"
			/>
		</label>
	{:else if fieldDef.type === 'number'}
		<div class="flex flex-col gap-1">
			<span class="text-sm">{label}</span>
			<div class="flex gap-2">
				<input
					type="number"
					min={fieldDef.min}
					max={fieldDef.max}
					value={Array.isArray(value) && typeof value[0] === 'number'
						? value[0]
						: (fieldDef.min ?? 0)}
					onchange={(e) => {
						const min = Number(e.currentTarget.value);
						const max =
							Array.isArray(value) && typeof value[1] === 'number'
								? value[1]
								: (fieldDef.max ?? 100);
						const newValue: FilterValue =
							min !== (fieldDef.min ?? 0) || max !== (fieldDef.max ?? 100) ? [min, max] : undefined;
						onChange?.(newValue);
					}}
					class="text-sm rounded border border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-800 px-2 py-1 w-20"
				/>
				<span class="text-sm self-center">to</span>
				<input
					type="number"
					min={fieldDef.min}
					max={fieldDef.max}
					value={Array.isArray(value) && typeof value[1] === 'number'
						? value[1]
						: (fieldDef.max ?? 100)}
					onchange={(e) => {
						const max = Number(e.currentTarget.value);
						const min =
							Array.isArray(value) && typeof value[0] === 'number' ? value[0] : (fieldDef.min ?? 0);
						const newValue: FilterValue =
							min !== (fieldDef.min ?? 0) || max !== (fieldDef.max ?? 100) ? [min, max] : undefined;
						onChange?.(newValue);
					}}
					class="text-sm rounded border border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-800 px-2 py-1 w-20"
				/>
			</div>
		</div>
	{/if}
</div>
