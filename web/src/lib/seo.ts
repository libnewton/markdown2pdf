// Absolute base URL for SEO meta tags. Discord, Slack, Twitter, etc. all
// require absolute URLs for og:image / twitter:image — relative paths are
// silently dropped.
//
// Override at build time with PUBLIC_SITE_URL if you deploy elsewhere.
import { PUBLIC_SITE_URL } from '$app/env/public';

export const SITE_URL = PUBLIC_SITE_URL;

export const SOCIAL_IMAGE = `${SITE_URL}/square.png`;
