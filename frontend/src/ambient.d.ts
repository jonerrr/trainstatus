// enhanced-img declares plain ?enhanced imports; custom transforms end in &enhanced.
declare module '*&enhanced' {
	import type { Picture } from '@sveltejs/enhanced-img';
	const image: Picture;
	export default image;
}
