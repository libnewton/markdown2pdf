// Absolute base URL for SEO meta tags. Discord, Slack, Twitter, etc. all
// require absolute URLs for og:image / twitter:image — relative paths are
// silently dropped.
//
// Override at build time with PUBLIC_SITE_URL if you deploy elsewhere.
import { env } from '$env/dynamic/public';

export const SITE_URL =
	(env.PUBLIC_SITE_URL || 'https://markdown2pdf.gitlab.io').replace(/\/$/, '');

export const SOCIAL_IMAGE = `${SITE_URL}/square.png`;
