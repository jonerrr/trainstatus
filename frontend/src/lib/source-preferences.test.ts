import { describe, expect, it } from 'vitest';

import { defaultSources, parseSources, serializeSources } from './source-preferences.svelte';

describe('parseSources', () => {
	it('falls back to subway and bus when the value is missing or blank', () => {
		expect(parseSources(null)).toEqual(defaultSources);
		expect(parseSources(undefined)).toEqual(defaultSources);
		expect(parseSources('')).toEqual(defaultSources);
		expect(parseSources('   ')).toEqual(defaultSources);
	});

	it('keeps valid sources and drops unknown tokens', () => {
		expect(parseSources('njt_bus, mta_subway')).toEqual(['njt_bus', 'mta_subway']);
		expect(parseSources('mta_bus, nope')).toEqual(['mta_bus']);
	});

	it('falls back to subway and bus when every token is invalid', () => {
		expect(parseSources('nope,also-invalid')).toEqual(defaultSources);
	});
});

describe('serializeSources', () => {
	it('joins sources with commas', () => {
		expect(serializeSources(['mta_subway', 'njt_bus'])).toBe('mta_subway,njt_bus');
	});
});
