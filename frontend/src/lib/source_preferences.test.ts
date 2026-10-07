import { describe, expect, it } from 'vitest';

import { default_sources, parse_sources, serialize_sources } from './source_preferences.svelte';

describe('parse_sources', () => {
	it('falls back to subway and bus when the value is missing or blank', () => {
		expect(parse_sources(null)).toEqual(default_sources);
		expect(parse_sources(undefined)).toEqual(default_sources);
		expect(parse_sources('')).toEqual(default_sources);
		expect(parse_sources('   ')).toEqual(default_sources);
	});

	it('keeps valid sources and drops unknown tokens', () => {
		expect(parse_sources('njt_bus, mta_subway')).toEqual(['njt_bus', 'mta_subway']);
		expect(parse_sources('mta_bus, nope')).toEqual(['mta_bus']);
	});

	it('falls back to subway and bus when every token is invalid', () => {
		expect(parse_sources('nope,also-invalid')).toEqual(default_sources);
	});
});

describe('serialize_sources', () => {
	it('joins sources with commas', () => {
		expect(serialize_sources(['mta_subway', 'njt_bus'])).toBe('mta_subway,njt_bus');
	});
});
