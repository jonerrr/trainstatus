import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	// Optional runtime origin; leave unset to use the incoming request origin.
	API_ORIGIN: { schema: (value) => value }
});
