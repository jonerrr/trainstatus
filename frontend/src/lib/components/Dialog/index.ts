import { createDialog, type CreateDialogProps, type Dialog as DialogType } from '@melt-ui/svelte';
import Trigger from './Trigger.svelte';
import Content from './Content.svelte';
import { pushState } from '$app/navigation';

// export const DIALOG_NAMES = ['settings', 'login', 'delete'] as const;

export type DialogName = string[][number];

function createDialogRegistry() {
	const registry = new Map<DialogName, DialogType>();

	function get(name: DialogName, props?: CreateDialogProps) {
		if (!registry.has(name)) {
			const dialog = createDialog({ ...props, closeOnOutsideClick: false });
			registry.set(name, dialog);
		}

		return registry.get(name) as DialogType;
	}

	function set(name: DialogName, dialog: DialogType) {
		registry.set(name, dialog);
	}

	// Shallow routing
	function shallow(name: DialogName, open: boolean) {
		if (open) {
			pushState('', {
				dialogOpen: open ? name : 'none'
			});
		}
	}

	return {
		get,
		set,
		shallow
	};
}

export const dialogRegistry = createDialogRegistry();

// If you need to predefine some props for a specific dialog, you can do it like this:
// dialogRegistry.set('delete', createDialog({ role: 'alertdialog', closeOnEscape: false }));

export const Dialog = {
	Trigger: Trigger,
	Content: Content
};
