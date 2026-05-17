<script lang="ts">
  import { onMount } from 'svelte'
  import { EditorView, basicSetup } from 'codemirror'
  import { EditorState } from '@codemirror/state'
  import { markdown as langMarkdown } from '@codemirror/lang-markdown'
  import { languages } from '@codemirror/language-data'
  import { oneDark } from '@codemirror/theme-one-dark'

  interface Props {
    markdown: string
    placeholder?: string
  }

  let { markdown = $bindable(), placeholder = '' }: Props = $props()

  let editorView = $state<EditorView | null>(null)
  let editorContainerEl = $state<HTMLDivElement | null>(null)
  let suppressEditorUpdate = false
  // Tracks the last document string the editor emitted upward. Lets the
  // external-sync effect bail without calling `doc.toString()` on every
  // keystroke — for large documents that string conversion alone is the
  // bulk of the per-keystroke cost.
  let lastEmittedDoc = ''

  export function insertTextAtSelection(text: string): boolean {
    if (!editorView) return false

    const { from, to } = editorView.state.selection.main
    suppressEditorUpdate = true
    editorView.dispatch({
      changes: { from, to, insert: text },
      selection: {
        anchor: from + text.length,
      },
      scrollIntoView: true,
    })
    suppressEditorUpdate = false
    const next = editorView.state.doc.toString()
    lastEmittedDoc = next
    markdown = next
    editorView.focus()
    return true
  }

  onMount(() => {
    if (!editorContainerEl) return
    lastEmittedDoc = markdown

    const startState = EditorState.create({
      doc: markdown,
      extensions: [
        basicSetup,
        langMarkdown({ codeLanguages: languages }),
        oneDark,
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !suppressEditorUpdate) {
            const next = update.state.doc.toString()
            lastEmittedDoc = next
            markdown = next
          }
        }),
        EditorView.theme({
          '&': {
            height: '100%',
            fontSize: '14px',
          },
          '.cm-scroller': {
            fontFamily: 'var(--font-mono)',
          },
        }),
        ...(placeholder
          ? [
              EditorView.contentAttributes.of({
                'aria-placeholder': placeholder,
              }),
            ]
          : []),
      ],
    })

    editorView = new EditorView({
      state: startState,
      parent: editorContainerEl,
    })

    return () => {
      editorView?.destroy()
    }
  })

  // Sync external markdown changes into the editor. We compare against the
  // last value the editor emitted upward, NOT against `doc.toString()` — the
  // latter is O(n) and runs on every keystroke through this effect, which is
  // the most expensive thing on the editor's hot path for large documents.
  $effect(() => {
    if (!editorView) return
    if (markdown === lastEmittedDoc) return
    lastEmittedDoc = markdown
    suppressEditorUpdate = true
    editorView.dispatch({
      changes: {
        from: 0,
        to: editorView.state.doc.length,
        insert: markdown,
      },
    })
    suppressEditorUpdate = false
  })
</script>

<div class="editor-host" bind:this={editorContainerEl}></div>

<style>
  .editor-host {
    flex: 1;
    height: 100%;
    overflow: hidden;
    background-color: #282c34;
    /* Isolate CodeMirror's internal layout from the rest of the page so
       typing doesn't invalidate layout/paint on the preview pane. */
    contain: strict;
  }

  .editor-host :global(.cm-editor) {
    height: 100%;
    outline: none;
  }
</style>
