<script lang="ts">
  import MarkdownEditor from '$lib/components/MarkdownEditor.svelte'
  import {
    getImageExtension,
    getImageAltText,
    escapeMarkdownImageAlt,
    createAssetId,
  } from '$lib/utils/image-utils'

  interface Props {
    markdown: string
    placeholder?: string
    errorMessage?: string | null
    readOnly?: boolean
    onImageSaved?: (path: string, bytes: Uint8Array<ArrayBuffer>, mimeType: string) => void
    onNewDocument?: () => void
    onScrolled?: () => void
  }

  let {
    markdown = $bindable(),
    placeholder = '',
    errorMessage = null,
    readOnly = false,
    onImageSaved,
    onNewDocument,
    onScrolled,
  }: Props = $props()

  let markdownEditor = $state<MarkdownEditor | null>(null)

  /**
   * Push the editor's buffered text upward now. The editor batches its
   * updates while typing, so anything that must act on the exact current
   * text (manual compile, export, save) flushes first.
   */
  export function flushPendingEdit(): void {
    markdownEditor?.flushPendingEdit()
  }

  export function setTaskMarker(line: number, checked: boolean): boolean {
    return markdownEditor?.setTaskMarker(line, checked) ?? false
  }

  export function topLine(): number | null {
    return markdownEditor?.topLine() ?? null
  }

  export function scrollToLine(line: number): void {
    markdownEditor?.scrollToLine(line)
  }

  export function insertMarkdownSnippet(snippet: string): void {
    if (markdownEditor?.insertTextAtSelection(snippet)) {
      return
    }

    const trimmed = markdown.trimEnd()
    markdown = trimmed ? `${trimmed}${snippet}` : snippet.trimStart()
  }

  async function saveLocalImage(file: File): Promise<string> {
    if (!file.type.startsWith('image/')) {
      throw new Error('Only image files are supported')
    }

    const path = `images/${createAssetId()}.${getImageExtension(file)}`
    const bytes = new Uint8Array(await file.arrayBuffer())
    const mimeType = file.type || 'application/octet-stream'

    onImageSaved?.(path, bytes, mimeType)

    return path
  }

  export async function insertImageFile(file: File): Promise<void> {
    try {
      const path = await saveLocalImage(file)
      const alt = escapeMarkdownImageAlt(getImageAltText(file))
      insertMarkdownSnippet(`\n\n![${alt}](${path})\n\n`)
    } catch (error) {
      // Errors are shown via errorMessage from parent
      console.error('Insert image failed:', error)
    }
  }
</script>

<MarkdownEditor
  bind:this={markdownEditor}
  bind:markdown
  {placeholder}
  {readOnly}
  {onNewDocument}
  {onScrolled}
/>
{#if errorMessage}
  <div class="error-bar">{errorMessage}</div>
{/if}

<style>
  .error-bar {
    padding: var(--space-sm, 8px) var(--space-md, 16px);
    font-size: 0.75rem;
    color: var(--color-danger);
    background: var(--color-danger-bg);
    border-top: 1px solid var(--color-danger);
  }
</style>
