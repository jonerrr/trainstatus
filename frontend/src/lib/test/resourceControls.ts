import type { LiveResource } from '../resources/index.svelte';
import type { UpdateStatus } from '../resources/status.svelte';
import type { StopTimeLiveResource } from '../resources/stop_times.svelte';
import type { createCurrentTime } from '../url_params.svelte';

export interface ResourceControls {
	resource: LiveResource<string[]>;
	arrivals: StopTimeLiveResource<'mta_bus'>;
	time: ReturnType<typeof createCurrentTime>;
	status: UpdateStatus;
}
