import { createSearchParamsSchema } from 'runed/kit';

// TODO: uninstall valibot since not needed (it wasnt working)
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
