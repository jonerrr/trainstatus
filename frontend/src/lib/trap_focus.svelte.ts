// from https://svelte.dev/tutorial/svelte/attach
import type { Attachment } from 'svelte/attachments';
import { on } from 'svelte/events';

// only HTMLElement has .focus method
export const trap_focus: Attachment<HTMLElement> = (node) => {
	const previous: HTMLElement | null = document.activeElement as HTMLElement;

	function focusable() {
		return Array.from(
			node.querySelectorAll(
				'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
			)
		) as HTMLElement[];
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key !== 'Tab') return;

		const current = document.activeElement;

		const elements = focusable();
		const first = elements.at(0);
		const last = elements.at(-1);

		if (event.shiftKey && current === first) {
			last?.focus();
			event.preventDefault();
		}

		if (!event.shiftKey && current === last) {
			first?.focus();
			event.preventDefault();
		}
	}

	focusable()[0]?.focus();
	const off = on(node, 'keydown', handleKeydown);

	return () => {
		off();
		previous?.focus();
	};
};
