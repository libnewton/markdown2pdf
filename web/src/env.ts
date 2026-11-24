import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_SITE_URL: {
		public: true,
		// Inlined at build time: the site is prerendered and has no runtime host.
		static: true,
		description: 'Absolute base URL for social-preview meta tags.',
		schema: (value) => (value || 'https://markdown2pdf.gitlab.io').replace(/\/$/, ''),
	},
});
