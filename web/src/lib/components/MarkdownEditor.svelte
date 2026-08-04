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
    readOnly?: boolean
  }

  let { markdown = $bindable(), placeholder = '', readOnly = false }: Props = $props()

  let editorView = $state<EditorView | null>(null)
  let editorContainerEl = $state<HTMLDivElement | null>(null)
  let suppressEditorUpdate = false
  // The last string handed upward. Also lets the external-sync effect below
  // bail out without calling `doc.toString()`.
  let lastEmittedDoc = ''
  // Handing the text upward is deferred: `doc.toString()` flattens
  // CodeMirror's rope into a fresh 100 kB+ string, and the write fans out to
  // autosave, the compile scheduler and the document menu. Nothing downstream
  // needs the text sooner than this.
  const EMIT_DELAY_MS = 150
  let emitTimer: number | null = null

  /** Hand any buffered edit upward now — before a compile, save or export. */
  export function flushPendingEdit() {
    if (emitTimer !== null) {
      clearTimeout(emitTimer)
      emitTimer = null
    }
    const next = editorView?.state.doc.toString()
    if (next === undefined || next === lastEmittedDoc) return
    lastEmittedDoc = next
    markdown = next
  }

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
    flushPendingEdit()
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
        // `readOnly` refuses the edit; `editable` also takes away the caret,
        // so the reference view never looks like something you can type in.
        EditorState.readOnly.of(readOnly),
        EditorView.editable.of(!readOnly),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged || suppressEditorUpdate) return
          if (emitTimer !== null) return
          emitTimer = window.setTimeout(() => {
            emitTimer = null
            flushPendingEdit()
          }, EMIT_DELAY_MS)
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
      if (emitTimer !== null) clearTimeout(emitTimer)
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
