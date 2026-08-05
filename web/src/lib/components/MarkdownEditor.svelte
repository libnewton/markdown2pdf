<script lang="ts">
  import { onMount } from 'svelte'
  import { EditorView, basicSetup } from 'codemirror'
  import { keymap } from '@codemirror/view'
  import { EditorState, EditorSelection } from '@codemirror/state'
  import { markdown as langMarkdown } from '@codemirror/lang-markdown'
  import { languages } from '@codemirror/language-data'
  import { oneDark } from '@codemirror/theme-one-dark'
  import { isolateHistory } from '@codemirror/commands'
  import { linkAround, slashCommand, toggleWrap } from '$lib/editor/commands'
  import { taskMarker } from '$lib/utils/task-marker'

  interface Props {
    markdown: string
    placeholder?: string
    readOnly?: boolean
    /** `/new` cannot be a snippet — it has to reach the document store. */
    onNewDocument?: () => void
    /** The editor was scrolled, so the preview can follow. */
    onScrolled?: () => void
  }

  let {
    markdown = $bindable(),
    placeholder = '',
    readOnly = false,
    onNewDocument,
    onScrolled,
  }: Props = $props()

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

  /**
   * Set the task marker on `line` (1-based) to `checked`.
   *
   * Returns false when that line no longer carries a marker — the preview was
   * stale, so nothing is written and the next render puts the box back.
   */
  export function setTaskMarker(line: number, checked: boolean): boolean {
    // `EditorState.readOnly` is a facet commands consult; it does not stop a
    // dispatch, so the reference view needs the guard spelled out.
    if (readOnly || !editorView) return false
    if (line < 1 || line > editorView.state.doc.lines) return false
    const l = editorView.state.doc.line(line)
    const marker = taskMarker(l.text)
    if (!marker) return false
    // Already right: the click was against a render that had fallen behind.
    if (marker.checked === checked) return true

    suppressEditorUpdate = true
    editorView.dispatch({
      changes: { from: l.from + marker.at, to: l.from + marker.at + 1, insert: checked ? 'x' : ' ' },
      // One toggle, one undo step: without this two quick ticks merge.
      annotations: isolateHistory.of('full'),
    })
    suppressEditorUpdate = false
    // Publish now rather than after the typing throttle, so the preview
    // catches up immediately. No `focus()`: clicking a box in the preview must
    // not pull the caret into a pane you may not be looking at.
    flushPendingEdit()
    return true
  }

  /** The 1-based source line at the top of the visible area. */
  export function topLine(): number | null {
    if (!editorView) return null
    const top = editorView.scrollDOM.getBoundingClientRect().top
    const pos = editorView.posAtCoords({ x: 0, y: top + 1 }, false)
    return editorView.state.doc.lineAt(pos).number
  }

  /** Put `line` at the top of the visible area. */
  export function scrollToLine(line: number) {
    if (!editorView) return
    const clamped = Math.min(Math.max(line, 1), editorView.state.doc.lines)
    const { from } = editorView.state.doc.line(clamped)
    editorView.dispatch({
      effects: EditorView.scrollIntoView(from, { y: 'start' }),
    })
  }

  /** Replace the selection, then place the caret where the helper asked. */
  function replaceSelection(
    view: EditorView,
    make: (selected: string) => { text: string; from: number; to: number },
  ): boolean {
    const range = view.state.selection.main
    const { text, from, to } = make(view.state.sliceDoc(range.from, range.to))
    view.dispatch({
      changes: { from: range.from, to: range.to, insert: text },
      selection: EditorSelection.range(range.from + from, range.from + to),
    })
    return true
  }

  const editorCommands = [
    { key: 'Mod-b', run: (v: EditorView) => replaceSelection(v, (s) => toggleWrap(s, '**')) },
    { key: 'Mod-i', run: (v: EditorView) => replaceSelection(v, (s) => toggleWrap(s, '_')) },
    { key: 'Mod-k', run: (v: EditorView) => replaceSelection(v, linkAround) },
    {
      // `/name` alone on a line, expanded on Enter. Deliberately not a
      // completion popup: the list is short and typing it out is the fast
      // path once you know it.
      key: 'Enter',
      run: (view: EditorView) => {
        const line = view.state.doc.lineAt(view.state.selection.main.head)
        const command = slashCommand(line.text)
        if (!command) return false
        if (command.name === 'new') {
          onNewDocument?.()
          view.dispatch({ changes: { from: line.from, to: line.to, insert: '' } })
          return true
        }
        view.dispatch({
          changes: { from: line.from, to: line.to, insert: command.snippet },
          selection: { anchor: line.from + command.caret },
        })
        return true
      },
    },
  ]

  onMount(() => {
    if (!editorContainerEl) return
    lastEmittedDoc = markdown

    const startState = EditorState.create({
      doc: markdown,
      extensions: [
        // Before basicSetup, so these win over its defaults.
        keymap.of(editorCommands),
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

    const notify = () => onScrolled?.()
    editorView.scrollDOM.addEventListener('scroll', notify, { passive: true })

    return () => {
      if (emitTimer !== null) clearTimeout(emitTimer)
      editorView?.scrollDOM.removeEventListener('scroll', notify)
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
