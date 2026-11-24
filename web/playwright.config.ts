import { defineConfig, devices } from '@playwright/test';

/**
 * The browser tests. `npm test` covers the pure pieces; these cover the thing
 * that actually ships, which nothing did before — the swallowed-headline bug
 * lived in the rendered document and no unit test could have seen it.
 */
export default defineConfig({
	testDir: 'e2e',
	// The first Typst compile loads several MB of WASM and fonts.
	timeout: 90_000,
	expect: { timeout: 20_000 },
	// Every test gets a fresh browser context, so they can run side by side.
	fullyParallel: true,
	workers: process.env.CI ? 4 : undefined,
	reporter: process.env.CI ? 'list' : [['list']],
	use: {
		baseURL: 'http://localhost:4173',
		...devices['Desktop Chrome'],
		launchOptions: {
			executablePath: process.env.PLAYWRIGHT_CHROMIUM ?? undefined,
		},
	},
	// Test the production build — what actually ships. Run `npm run build` first.
	webServer: {
		command: 'npm run preview -- --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 120_000,
	},
});
