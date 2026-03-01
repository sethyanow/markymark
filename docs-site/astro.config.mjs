// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'markymark',
			description: 'High-performance Markdown LSP and MCP server',
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/sethyanow/markymark' },
			],
			sidebar: [
				{ label: 'About', slug: 'about' },
				{
					label: 'Getting Started',
					items: [
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Quick Start', slug: 'getting-started/quick-start' },
					],
				},
				{
					label: 'Usage',
					items: [
						{ label: 'Workspace Management', slug: 'usage/workspace-management' },
						{ label: 'Navigation', slug: 'usage/navigation' },
						{ label: 'Diagnostics', slug: 'usage/diagnostics' },
						{ label: 'Refactoring', slug: 'usage/refactoring' },
						{ label: 'Search', slug: 'usage/search' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Using with AI Agents', slug: 'guides/agents' },
					],
				},
				{
					label: 'Editors',
					items: [
						{ label: 'VS Code', slug: 'editors/vscode' },
						{ label: 'Neovim', slug: 'editors/neovim' },
						{ label: 'Claude Code', slug: 'editors/claude-code' },
					],
				},
				{
					label: 'Features',
					items: [
						{ label: 'LSP Capabilities', slug: 'features/lsp' },
						{ label: 'MCP Tools Reference', slug: 'features/mcp-tools' },
						{ label: 'Supported Formats', slug: 'features/supported-formats' },
					],
				},
				{
					label: 'Architecture',
					items: [
						{ label: 'Overview', slug: 'architecture/overview' },
						{ label: 'Parser Pipeline', slug: 'architecture/parser-pipeline' },
						{ label: 'Indexing', slug: 'architecture/indexing' },
					],
				},
				{
					label: 'Contributing',
					items: [
						{ label: 'Development Setup', slug: 'contributing/development' },
						{ label: 'Project Structure', slug: 'contributing/project-structure' },
						{ label: 'Guidelines', slug: 'contributing/guidelines' },
					],
				},
				{ label: 'Troubleshooting', slug: 'troubleshooting' },
				{ label: 'FAQ', slug: 'faq' },
				{ label: 'Changelog', slug: 'changelog' },
			],
		}),
	],
});
