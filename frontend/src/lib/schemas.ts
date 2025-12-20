import * as v from 'valibot';

export const search_schema = v.object({
	s: v.optional(v.number()),
	r: v.optional(v.string()),
	t: v.optional(v.string()),
	at: v.optional(
		// v.pipe(
		v.number()
		// 	v.transform((val) => new Date(val * 1000))
		// )
	)
});
