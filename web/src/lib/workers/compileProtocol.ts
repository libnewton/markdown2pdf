/**
 * Shared between the Typst worker and its client.
 *
 * Marker for a preview compile the worker dropped because a newer one arrived.
 * It travels as an error so the pending promise always settles, but it is not
 * a failure: the caller ignores it rather than showing a compile error.
 */
export const SUPERSEDED = 'md2pdf:superseded';
