import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy'
import path from 'node:path'

export default defineConfig({
	plugins: [
		sveltekit(),
		viteStaticCopy({
      targets: [
        {
          src: path.resolve(__dirname, './examples') + '/[!.]*', // 1️⃣
          dest: './examples', // 2️⃣
        },
      ],
    }),
	],
	server: {
		fs: {
			strict: false
		}
	}
});
