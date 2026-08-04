import { browser } from '$app/environment';

const LIVE_UPDATE_KEY = 'md2pdf-live-update';
export const THEME_KEY = 'md2pdf-theme';

export type Theme = 'light' | 'dark';

function readBool(key: string, fallback: boolean): boolean {
	if (!browser) return fallback;
	try {
		const v = localStorage.getItem(key);
		if (v === null) return fallback;
		return v !== 'false';
	} catch {
		return fallback;
	}
}

function writeBool(key: string, value: boolean) {
	if (!browser) return;
	try {
		localStorage.setItem(key, value ? 'true' : 'false');
	} catch {
		// ignore
	}
}

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
	liveUpdate = $state(readBool(LIVE_UPDATE_KEY, true));
	theme = $state<Theme>(initialTheme());

	setLiveUpdate(value: boolean) {
		this.liveUpdate = value;
		writeBool(LIVE_UPDATE_KEY, value);
	}

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
