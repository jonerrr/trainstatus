import type { HandleClientError } from '@sveltejs/kit/hooks';

import { handleFrontendError } from '#lib/errors.js';

export const handleError: HandleClientError = handleFrontendError;
