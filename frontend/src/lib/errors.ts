import type { CaughtError } from '@sveltejs/kit/hooks';

/** Kit uses this for route failures and transforms rendering errors before boundaries see them. */
export function handleFrontendError({ kind, error }: CaughtError) {
	if (kind !== 'unknown') return;
	console.error('Unhandled frontend error', error);
	return { message: 'Something went wrong while loading this page. Please try again.' };
}
