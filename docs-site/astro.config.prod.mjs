import config from './astro.config.mjs';
import remarkBaseLinks from './src/plugins/remark-base-links.mjs';

export default {
	...config,
	site: 'https://sethyanow.github.io',
	base: '/markymark',
	markdown: {
		remarkPlugins: [[remarkBaseLinks, { base: '/markymark' }]],
	},
};
