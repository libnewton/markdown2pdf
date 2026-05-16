import { browser } from '$app/environment';

const LIVE_UPDATE_KEY = 'md2pdf-live-update';
const PAGE_NUMBERS_KEY = 'md2pdf-page-numbers';
const CORS_PROXY_KEY = 'md2pdf-cors-proxy';

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

function readString(key: string, fallback: string): string {
	if (!browser) return fallback;
	try {
		return localStorage.getItem(key) ?? fallback;
	} catch {
		return fallback;
	}
}

function writeString(key: string, value: string) {
	if (!browser) return;
	try {
		if (value) localStorage.setItem(key, value);
		else localStorage.removeItem(key);
	} catch {
		// ignore
	}
}

class SettingsStore {
	liveUpdate = $state(readBool(LIVE_UPDATE_KEY, true));
	pageNumbers = $state(readBool(PAGE_NUMBERS_KEY, true));
	corsProxy = $state(readString(CORS_PROXY_KEY, ''));

	setLiveUpdate(value: boolean) {
		this.liveUpdate = value;
		writeBool(LIVE_UPDATE_KEY, value);
	}

	setPageNumbers(value: boolean) {
		this.pageNumbers = value;
		writeBool(PAGE_NUMBERS_KEY, value);
	}

	setCorsProxy(value: string) {
		const trimmed = value.trim();
		this.corsProxy = trimmed;
		writeString(CORS_PROXY_KEY, trimmed);
	}
}

export const settingsStore = new SettingsStore();
