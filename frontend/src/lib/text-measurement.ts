// Singleton canvas context — avoids memory leaks and DOM overhead.
let measureCtx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null = null;

function getContext() {
	if (measureCtx) return measureCtx;

	// Prefer OffscreenCanvas (faster, no DOM required), fallback to standard Canvas
	if (typeof OffscreenCanvas !== 'undefined') {
		measureCtx = new OffscreenCanvas(1, 1).getContext('2d')!;
	} else if (typeof document !== 'undefined') {
		measureCtx = document.createElement('canvas').getContext('2d')!;
	}
	return measureCtx!;
}

/**
 * Calculates the exact height of wrapped text using canvas text measurement.
 *
 * @param text The string to measure
 * @param font The exact CSS font string (e.g., "500 16px Inter, sans-serif")
 * @param maxWidth The maximum width the text is allowed to take up in pixels
 * @param lineHeight The pixel height of a single line of text (e.g., 24)
 */
export function calculateTextHeight(
	text: string,
	font: string,
	maxWidth: number,
	lineHeight: number
): number {
	if (!text) return 0;

	const ctx = getContext();
	if (!ctx) return lineHeight; // SSR fallback — assume 1 line
	ctx.font = font;

	const words = text.split(' ');
	let lineCount = 1;
	let currentLineWidth = 0;

	for (const word of words) {
		const wordWidth = ctx.measureText(word + ' ').width;

		// If a single word is wider than the container it forces a break
		if (wordWidth > maxWidth) {
			lineCount++;
			currentLineWidth = 0;
			continue;
		}

		if (currentLineWidth + wordWidth > maxWidth) {
			lineCount++;
			currentLineWidth = wordWidth;
		} else {
			currentLineWidth += wordWidth;
		}
	}

	return lineCount * lineHeight;
}
