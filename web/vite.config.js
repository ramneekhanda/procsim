import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy'
import path from 'node:path'

export default defineConfig({
	plugins: [
		sveltekit()
	],
	server: {
		fs: {
			strict: false
		}
	}
});
