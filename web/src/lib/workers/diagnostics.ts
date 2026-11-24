// Typst reports against the files it compiled: the generated entry point and
// the md2pdf package. Neither location means anything to the author, whose
// document is Markdown, so they are stripped and only the message is kept.
const INTERNAL_LOCATION = /^\/?(?:md2pdf\/\S*?|main\.typ|doc\.md):\d+:\d+:\s*/;

// The package names fallback families the bundled fonts do not have. Typst
// warns once per family, which buried the one line that mattered.
const FONT_WARNING = /^warning: unknown font family/;

// Call-stack hints point into the package's own code.
const INTERNAL_HINT = /^hint: (?:while calling|error occurred in this call)/;

/** A compile failure as the author should read it: one message per line. */
export function formatCompileError(diagnostics: readonly string[]): string {
	const lines = diagnostics
		.map((d) => d.trim().replace(INTERNAL_LOCATION, ''))
		.filter((d) => !FONT_WARNING.test(d))
		.flatMap((d) => d.split('\n'))
		.map((line) => line.trim().replace(INTERNAL_LOCATION, ''))
		.filter((line) => line !== '' && !INTERNAL_HINT.test(line));
	return [...new Set(lines)].join('\n');
}
