import type { Source } from '#lib/client/index.js';

import '../../app.css';

import { page } from '#lib/test/page.svelte.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import ChartsPage from '../../routes/charts/+page.svelte';
import { resources, setFixture, stopResources } from './set-fixture.svelte.js';

vi.mock('#lib/url_params.svelte.js', async () => ({
	...(await import('./set-fixture.svelte.js')),
	open_modal: () => {}
}));
vi.mock('#lib/resources/trips.svelte.js', () => ({ trip_context: { get: () => resources } }));
vi.mock('#lib/resources/stop_times.svelte.js', () => ({
	stop_time_context: { get: () => stopResources }
}));
vi.mock('#lib/resources/alerts.svelte.js', () => ({ alert_context: { get: () => ({}) } }));

async function renderChart() {
	const { container } = await render(ChartsPage);
	container.style.width = '1400px';
	container.style.height = '800px';
}

async function expectChartSize() {
	await expect
		.poll(
			() => document.querySelector('.layercake-layout-svg')?.getBoundingClientRect().height ?? 0
		)
		.toBeGreaterThan(400);
}

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
	for (const direction of [0, 1]) {
		test(`${source} direction ${direction}: every rendered trip progresses through the stop axis`, async () => {
			const fixture = setFixture(source, direction);
			await renderChart();
			if (source !== 'mta_subway') {
				await userEvent.click(document.querySelector('[aria-label="Add routes"]')!);
				await userEvent.click(document.querySelector('[role="option"]')!);
			}
			if (direction === 1) await userEvent.click(document.querySelector('#southbound')!);
			await expect
				.poll(() => document.querySelectorAll('.path-line').length)
				.toBe(fixture.trips.length);
			await expectChartSize();
			for (const [index, path] of [...document.querySelectorAll('.path-line')].entries()) {
				const d = path.getAttribute('d')!;
				expect(d).not.toMatch(/NaN|undefined/);
				const points = [...d.matchAll(/[ML](-?[\d.]+),(-?[\d.]+)/g)].map((m) => [
					Number(m[1]),
					Number(m[2])
				]);
				expect(points.length).toBe(fixture.stopTimes.get(fixture.trips[index].id)!.length);
				expect(
					points.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y) && x >= 0 && y >= 0)
				).toBe(true);
				for (let i = 1; i < points.length; i++) {
					expect.soft(points[i][0]).toBeGreaterThan(points[i - 1][0]);
					expect.soft(points[i][1]).toBeGreaterThan(points[i - 1][1]);
				}
			}
			expect(document.querySelectorAll('.stop-name').length).toBe(5);
			await userEvent.click(document.querySelector('#stop_points')!);
			expect(document.querySelectorAll('.layercake-layout-svg circle').length).toBe(11);
		});

		test(`${source} direction ${direction}: loop visits render as separate axis rows`, async () => {
			setFixture(source, direction, [
				['a', 'b', 'a', 'c'],
				['a', 'c', 'b']
			]);
			await renderChart();
			if (source !== 'mta_subway') {
				await userEvent.click(document.querySelector('[aria-label="Add routes"]')!);
				await userEvent.click(document.querySelector('[role="option"]')!);
			}
			if (direction === 1) await userEvent.click(document.querySelector('#southbound')!);
			await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(2);
			await expectChartSize();
			for (const path of document.querySelectorAll('.path-line')) {
				const points = [...path.getAttribute('d')!.matchAll(/[ML](-?[\d.]+),(-?[\d.]+)/g)].map(
					(m) => [Number(m[1]), Number(m[2])]
				);
				for (let i = 1; i < points.length; i++) {
					expect(points[i][0]).toBeGreaterThan(points[i - 1][0]);
					expect(points[i][1]).toBeGreaterThan(points[i - 1][1]);
				}
			}
			expect(
				[...document.querySelectorAll('.stop-name')].some((label) =>
					label.textContent?.includes('(2)')
				)
			).toBe(true);
			await userEvent.click(document.querySelector('#stop_points')!);
			expect(document.querySelectorAll('.layercake-layout-svg circle').length).toBe(7);
		});
	}
}

test('multiple sources keep colliding trip/stop/route IDs separate and use their own route colors', async () => {
	const sources: Source[] = ['mta_subway', 'mta_bus', 'njt_bus'];
	const snapshots = sources.map((source) => {
		setFixture(source, 0);
		return page.data;
	});
	page.data = {
		selected_sources: sources,
		stops: Object.assign({}, ...snapshots.map((data) => data.stops)),
		stops_by_id: Object.assign({}, ...snapshots.map((data) => data.stops_by_id)),
		routes: Object.assign({}, ...snapshots.map((data) => data.routes)),
		routes_by_id: Object.assign({}, ...snapshots.map((data) => data.routes_by_id))
	};
	await renderChart();
	for (const source of ['mta_bus', 'njt_bus']) {
		await userEvent.click(document.querySelector('[aria-label="Add routes"]')!);
		const option = [...document.querySelectorAll('[role="option"]')].find((option) =>
			option.textContent?.includes(`${source} route`)
		)!;
		await userEvent.click(option);
	}
	await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(9);
	await expectChartSize();
	expect(
		[...document.querySelectorAll('.path-line')].map((path) => path.getAttribute('stroke'))
	).toEqual([
		'#00933c',
		'#00933c',
		'#00933c',
		'#123456',
		'#123456',
		'#123456',
		'#abcdef',
		'#abcdef',
		'#abcdef'
	]);
	expect(document.querySelectorAll('.stop-name').length).toBe(15);
});
