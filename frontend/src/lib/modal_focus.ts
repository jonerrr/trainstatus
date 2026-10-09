import type { Attachment } from 'svelte/attachments';
import { on } from 'svelte/events';

/** Supply modal interaction without a native dialog's browser close watcher. */
export const contain_modal_focus: Attachment<HTMLElement> = (node) => {
	const previous_focus = document.activeElement;
	const previous_overflow = document.body.style.overflow;
	const inert_elements: Array<{ element: HTMLElement; inert: boolean }> = [];

	// Inert siblings along the entire ancestor path, including the navbar
	// outside <main>, without making the sheet itself inert.
	for (
		let branch: HTMLElement | null = node;
		branch?.parentElement;
		branch = branch.parentElement
	) {
		for (const sibling of branch.parentElement.children) {
			if (sibling === branch || !(sibling instanceof HTMLElement)) continue;
			inert_elements.push({ element: sibling, inert: sibling.inert });
			sibling.inert = true;
		}
		if (branch.parentElement === document.body) break;
	}
	document.body.style.overflow = 'hidden';

	function focusable() {
		return Array.from(
			node.querySelectorAll<HTMLElement>(
				'button, [href], input, select, textarea, summary, [tabindex]'
			)
		).filter(
			(element) =>
				element.tabIndex >= 0 &&
				!element.matches(':disabled') &&
				!element.closest('[inert]') &&
				element.getClientRects().length > 0
		);
	}

	function focus_inside() {
		(focusable()[0] ?? node).focus({ preventScroll: true });
	}
	function handle_focus(event: FocusEvent) {
		if (event.target instanceof Node && !node.contains(event.target)) focus_inside();
	}
	function handle_tab(event: KeyboardEvent) {
		if (event.key !== 'Tab') return;
		const elements = focusable();
		const first = elements[0];
		const last = elements.at(-1);
		if (
			!first ||
			(event.shiftKey
				? document.activeElement === first || document.activeElement === node
				: document.activeElement === last || document.activeElement === node)
		) {
			event.preventDefault();
			(event.shiftKey ? (last ?? node) : (first ?? node)).focus({ preventScroll: true });
		}
	}

	const stop_focus = on(document, 'focusin', handle_focus);
	const stop_tab = on(node, 'keydown', handle_tab);
	focus_inside();

	return () => {
		stop_focus();
		stop_tab();
		for (const { element, inert } of inert_elements) element.inert = inert;
		document.body.style.overflow = previous_overflow;
		if (previous_focus instanceof HTMLElement && previous_focus.isConnected) {
			previous_focus.focus({ preventScroll: true });
		}
	};
};
