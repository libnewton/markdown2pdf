/** Whether a key event came from somewhere text is being entered. */
export function isTyping(target: EventTarget | null): boolean {
	const el = target as HTMLElement | null;
	if (!el?.tagName) return false;
	return (
		el.isContentEditable ||
		['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) ||
		!!el.closest?.('.cm-editor')
	);
}

export type ShortcutActions = {
	/** Something Escape should close is open. */
	hasOpenOverlay: () => boolean;
	closeOverlays: () => void;
	toggleShortcuts: () => void;
	compileNow: () => void;
	/** Absent in the reference view, which has no documents. */
	newDocument?: () => void;
	toggleExport: () => void;
	exportPdf: () => void;
	/** 0, 1 or 2: editor only, split, document only. */
	setViewMode: (index: number) => void;
	togglePreviewMode: () => void;
	toggleOutline: () => void;
};

/**
 * The app-wide keyboard shortcuts, for a capture-phase `keydown` listener:
 * a shortcut claimed here never reaches CodeMirror's own keymap, which could
 * otherwise, say, insert a newline for Ctrl+Enter as well as compiling.
 */
export function shortcutHandler(actions: ShortcutActions): (e: KeyboardEvent) => void {
	return (e) => {
		const mod = e.ctrlKey || e.metaKey;
		const claim = () => {
			e.preventDefault();
			e.stopImmediatePropagation();
			e.stopPropagation();
		};

		if (e.key === 'Escape') {
			if (!actions.hasOpenOverlay()) return;
			claim();
			actions.closeOverlays();
			return;
		}
		// `?` where you are not typing — the usual way to ask for this list.
		if (!mod && e.key === '?' && !isTyping(e.target)) {
			claim();
			actions.toggleShortcuts();
			return;
		}
		if (!mod) return;

		switch (e.key.toLowerCase()) {
			case 'enter':
				claim();
				actions.compileNow();
				return;
			case 'n':
				// Cmd/Ctrl+N belongs to the browser, so this one takes Alt too.
				if (!e.altKey || !actions.newDocument) return;
				claim();
				actions.newDocument();
				return;
			case 's':
				// Swallowed rather than handled: there is nothing to save, and the
				// browser's "save page as" is not what anyone means by it here.
				claim();
				return;
			case 'e':
				claim();
				actions.toggleExport();
				return;
			case 'p':
				claim();
				actions.exportPdf();
				return;
			case '1':
			case '2':
			case '3':
				claim();
				actions.setViewMode(Number(e.key) - 1);
				return;
			case '\\':
				claim();
				actions.togglePreviewMode();
				return;
			case 'o':
				claim();
				actions.toggleOutline();
				return;
			case '/':
				claim();
				actions.toggleShortcuts();
				return;
		}
	};
}
