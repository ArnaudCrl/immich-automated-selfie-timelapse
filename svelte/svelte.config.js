import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	// Consult https://svelte.dev/docs/kit/integrations
	// for more information about preprocessors
	preprocess: vitePreprocess(),

	kit: {
		adapter: adapter({
			pages: '../templates',
			assets: '../templates',
			fallback: undefined,
			precompress: false,
			strict: true
		}),
		paths: {
			base: '',
			assets: ''
		}
	}
};

export default config;
