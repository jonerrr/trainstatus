import '../../app.css';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';

import PlatformExits from './PlatformExits.svelte';

for (const width of [320, 390, 800])
	for (const long_names of [false, true]) {
		test(`${long_names ? 'long' : 'short'} named exit labels fit and stay separate at ${width}px`, async () => {
			await page.viewport(width, 844);
			const view = await render(PlatformExits, {
				guidance: {
					car_count: 10,
					exits: [
						{
							id: 'rear',
							name: long_names
								? 'Westchester Avenue & SB Bruckner Blvd/Amendola Plaza'
								: 'Utica Av',
							accessible: false,
							car: 10,
							position: 0.0283
						},
						{
							id: 'elevator',
							name: long_names
								? 'Westchester Avenue & SB Bruckner Blvd/Amendola Plaza'
								: 'Utica Av',
							accessible: true,
							car: 8,
							position: 0.2567
						},
						{
							id: 'front',
							name: long_names
								? 'Penn Station Hilton Passageway (LIRR, NJ Transit, Amtrak)'
								: 'Stuyvesant Av',
							accessible: false,
							car: 1,
							position: 0.9783
						}
					]
				}
			});
			await expect
				.poll(() => {
					const section = view
						.getByRole('region', { name: 'Platform exits at arrival' })
						.element()
						.getBoundingClientRect();
					const boxes = view
						.getByRole('button')
						.all()
						.map((button) => button.element().getBoundingClientRect());
					if (
						boxes.some(
							(box) =>
								box.left < section.left || box.right > section.right || box.bottom > section.bottom
						)
					)
						return false;
					for (let i = 0; i < boxes.length; i++)
						for (let j = i + 1; j < boxes.length; j++) {
							const a = boxes[i],
								b = boxes[j];
							if (!(
								a.right <= b.left ||
								b.right <= a.left ||
								a.bottom <= b.top ||
								b.bottom <= a.top
							))
								return false;
						}
					return true;
				})
				.toBe(true);
			await view.unmount();
		});
	}

for (const car_count of [4, 10]) {
	test(`${car_count}-car trains keep compact cars and vertical exit arrows in wide modals`, async () => {
		await page.viewport(1000, 844);
		const view = await render(PlatformExits, {
			guidance: {
				car_count,
				exits: [
					{ id: 'end', name: 'Barclays Center', accessible: false, car: car_count, position: 0 },
					{
						id: 'stairs',
						name: 'Pacific St & 4 Av',
						accessible: false,
						car: Math.ceil(car_count / 2),
						position: 0.52
					},
					{
						id: 'lift',
						name: 'Pacific St & 4 Av',
						accessible: true,
						car: Math.ceil(car_count * 0.4),
						position: 0.61
					}
				]
			}
		});
		await expect
			.poll(() => {
				const cars = [...document.querySelectorAll('[data-train-car]')].map((car) =>
					car.getBoundingClientRect()
				);
				return (
					cars.length === car_count &&
					cars.every((car) => car.width <= 36 && car.height >= 23 && car.height <= 25)
				);
			})
			.toBe(true);
		const arrows = [...document.querySelectorAll('[data-exit-shaft]')];
		expect(arrows).toHaveLength(3);
		for (const arrow of arrows) expect(arrow.getBoundingClientRect().width).toBe(0);
		const rear = document.querySelector('[data-train-car]')!.getBoundingClientRect().left;
		expect(Math.abs(arrows[0].getBoundingClientRect().left - rear)).toBeLessThanOrEqual(2);
		await page.viewport(390, 844);
		await expect
			.poll(() => {
				const cars = [...document.querySelectorAll('[data-train-car]')].map((car) =>
					car.getBoundingClientRect()
				);
				const rear = cars[0].left;
				return (
					cars.every((car) => car.width <= 36 && car.height >= 23 && car.height <= 25) &&
					Math.abs(arrows[0].getBoundingClientRect().left - rear) <= 2
				);
			})
			.toBe(true);
		await view.unmount();
	});
}
