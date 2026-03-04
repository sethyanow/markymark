/**
 * Remark plugin that prefixes internal absolute links with Astro's `base` path.
 *
 * Starlight auto-prefixes sidebar links, but authored markdown links (e.g.,
 * `[text](/getting-started/installation/)`) are not rewritten. This plugin
 * bridges that gap for subpath deployments.
 *
 * Usage in astro.config.prod.mjs:
 *   markdown: { remarkPlugins: [[remarkBaseLinks, { base: '/markymark' }]] }
 *
 * Only affects links starting with `/` that aren't already prefixed.
 * Safe to include in dev config (no-op when base is empty).
 */
import { visit } from 'unist-util-visit';

export default function remarkBaseLinks(options = {}) {
	const base = (options.base || '').replace(/\/$/, '');

	return (tree) => {
		if (!base) return;

		visit(tree, ['link', 'image', 'definition'], (node) => {
			const alreadyPrefixed =
				node.url === base ||
				node.url.startsWith(`${base}/`) ||
				node.url.startsWith(`${base}?`) ||
				node.url.startsWith(`${base}#`);

			if (
				node.url.startsWith('/') &&
				!alreadyPrefixed &&
				!node.url.startsWith('//')
			) {
				node.url = `${base}${node.url}`;
			}
		});
	};
}
