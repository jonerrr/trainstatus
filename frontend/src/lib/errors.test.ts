import { afterEach, expect, test, vi } from 'vitest';

import { handleFrontendError } from './errors';

afterEach(() => vi.restoreAllMocks());

test('unexpected errors are logged but their raw message is not exposed', () => {
	const log = vi.spyOn(console, 'error').mockImplementation(() => {});
	const error = new Error('Private diagnostic detail');
	const result = handleFrontendError({ kind: 'unknown', error });
	expect(log).toHaveBeenCalledWith('Unhandled frontend error', error);
	expect(result?.message).toContain('Please try again');
	expect(result?.message).not.toContain(error.message);
});

test('app and framework errors retain Kit defaults without unexpected-error logging', () => {
	const log = vi.spyOn(console, 'error').mockImplementation(() => {});
	expect(
		handleFrontendError({ kind: 'app', error: { status: 400, message: 'Invalid time' } })
	).toBeUndefined();
	expect(
		handleFrontendError({ kind: 'framework', error: { status: 404, message: 'Not Found' } })
	).toBeUndefined();
	expect(log).not.toHaveBeenCalled();
});
