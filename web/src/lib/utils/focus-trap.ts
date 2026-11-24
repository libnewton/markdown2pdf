const FOCUSABLE =
	'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])';

/**
 * A modal dialog's focus handling, as a Svelte attachment: focus moves into
 * the dialog, Tab and Shift+Tab cycle inside it, and focus returns to where it
 * was once the dialog goes away.
 */
export function trapFocus(dialog: HTMLElement): () => void {
	const returnTo = document.activeElement as HTMLElement | null;
	const focusables = () => [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)];
	(focusables()[0] ?? dialog).focus();

	const onKeydown = (e: KeyboardEvent) => {
		if (e.key !== 'Tab') return;
		const items = focusables();
		if (items.length === 0) {
			e.preventDefault();
			return;
		}
		const first = items[0];
		const last = items[items.length - 1];
		const active = document.activeElement;
		if (e.shiftKey && (active === first || !dialog.contains(active))) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && (active === last || !dialog.contains(active))) {
			e.preventDefault();
			first.focus();
		}
	};
	dialog.addEventListener('keydown', onKeydown);
	return () => {
		dialog.removeEventListener('keydown', onKeydown);
		returnTo?.focus?.();
	};
}
