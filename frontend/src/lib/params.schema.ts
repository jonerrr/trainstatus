import { createSearchParamsSchema } from 'runed/kit';

export const searchSchema = createSearchParamsSchema({
	// route
	r: {
		type: 'string'
	},
	// stop
	s: {
		type: 'number'
	},
	// trip
	t: {
		type: 'string'
	},
	// unix timestamp in seconds
	at: {
		type: 'number'
	}
});
