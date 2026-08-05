import { browser } from '$app/environment';

export const THEME_KEY = 'md2pdf-theme';

export type Theme = 'light' | 'dark';

/**
 * The stored preference, or the system one on a first visit. Resolved once:
 * from then on the choice is the user's, so a change in the OS setting does
 * not move a document that is already open.
 */
function initialTheme(): Theme {
	if (!browser) return 'light';
	try {
		const stored = localStorage.getItem(THEME_KEY);
		if (stored === 'light' || stored === 'dark') return stored;
	} catch {
		// ignore
	}
	return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

class SettingsStore {
	theme = $state<Theme>(initialTheme());

	setTheme(value: Theme) {
		this.theme = value;
		if (!browser) return;
		document.documentElement.dataset.theme = value;
		try {
			localStorage.setItem(THEME_KEY, value);
		} catch {
			// ignore
		}
	}
}

export const settingsStore = new SettingsStore();
