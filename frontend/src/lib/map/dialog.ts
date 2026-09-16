export function dismissOnEscape(event: KeyboardEvent, dismiss: () => void): boolean {
	if (event.key !== 'Escape') return false;
	event.preventDefault();
	dismiss();
	return true;
}
