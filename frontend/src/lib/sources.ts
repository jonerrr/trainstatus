import type { Source } from '@trainstatus/client';

export const default_sources: Source[] = ['mta_bus', 'mta_subway'] as const;
// TODO: allow changing sources
