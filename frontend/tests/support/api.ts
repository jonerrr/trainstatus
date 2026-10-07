export const API_ORIGIN = 'http://127.0.0.1:3055';

export async function readJson<T>(path: string): Promise<T> {
	const url = new URL(path, API_ORIGIN);
	let response: Response;
	try {
		response = await fetch(url, { signal: AbortSignal.timeout(20_000) });
	} catch (error) {
		throw new Error(`Failed to fetch ${url}`, { cause: error });
	}
	if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
	return response.json() as Promise<T>;
}
