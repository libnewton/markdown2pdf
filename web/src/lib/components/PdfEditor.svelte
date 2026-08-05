<script lang="ts">
  import { browser } from '$app/environment'
  import { page } from '$app/state'
  import { replaceState } from '$app/navigation'
  import { onMount } from 'svelte'
  import { pageSlots } from '$lib/preview/page-slots'
  import type { SvgDocument } from '$lib/typst/svg-split'
  import { getSharedTypstWorkerClient, TypstWorkerClient } from '$lib/workers/typstClient'
  import { getMarkdownImportFile, getImageDropFile } from '$lib/utils/image-utils'
  import { PAGEBREAK_TOKEN } from '$lib/pagebreak'
  import { cachedRemoteImages, prefetchRemoteImages } from '$lib/utils/remote-images'
  import { SUPERSEDED } from '$lib/workers/compileProtocol'
  import { useRegisterSW } from 'virtual:pwa-register/svelte'
  import type { RegisterSWOptions } from 'virtual:pwa-register/svelte'
  import { writable, type Readable } from 'svelte/store'

  import StatusHint from '$lib/components/StatusHint.svelte'
  import HtmlPreview from '$lib/components/HtmlPreview.svelte'
  import ShortcutOverlay from '$lib/components/ShortcutOverlay.svelte'
  import EditorPane from '$lib/components/EditorPane.svelte'
  import DocumentMenu from '$lib/components/DocumentMenu.svelte'
  import { PDF_TEMPLATES } from '$lib/templates/pdf-templates'
  import {
    deriveNameFromContent,
    documentStore,
    isBrokenTemplateDocument,
    isLegacyImplicitBlankDocument,
  } from '$lib/stores/documentStore.svelte'
  import { settingsStore } from '$lib/stores/settingsStore.svelte'
  import { SOCIAL_IMAGE } from '$lib/seo'

  import type { SavedDocument, SavedDocumentAsset } from '$lib/storage/documents'

  // Props
  interface Props {
    seoTitle: string
    seoDescription: string
    initialMarkdown?: string
    /** The reference view: nothing is edited, nothing is saved. */
    readOnly?: boolean
  }

  let { seoTitle, seoDescription, initialMarkdown = '', readOnly = false }: Props = $props()

  let markdown = $state('')
  let hasInitializedMarkdown = false
  let editorPane = $state<EditorPane | null>(null)

  type LocalImageAsset = {
    bytes: Uint8Array<ArrayBuffer>
    mimeType: string
  }

  let imageAssets = $state<Record<string, LocalImageAsset>>({})
  // Images handed to the Typst worker, keyed by their path in the document.
  type ImageMap = Record<string, Uint8Array<ArrayBuffer>>

  // What gets stored in IndexedDB. Unproxied — IndexedDB cannot clone a
  // `$state` proxy — and a fresh identity on every upload, which is how the
  // autosave below knows the bytes need writing again.
  const documentAssets = $derived(
    $state.snapshot(imageAssets) as Record<string, SavedDocumentAsset>,
  )

  // PWA Service Worker — useRegisterSW touches `navigator` at setup time, so
  // we can only call it in the browser. SSR/prerender gets a fallback store
  // that is always false; the `{#if $needRefresh}` template branch stays
  // dormant until a real registration happens client-side.
  let swUpdateTimer: ReturnType<typeof setInterval> | null = null
  const swOptions: RegisterSWOptions = {
    onRegistered(swr) {
      if (!swr) return
      swUpdateTimer = setInterval(() => void swr.update(), 60 * 60 * 1000)
    },
    onRegisterError(error) {
      console.error('service worker registration failed', error)
    },
  }
  let needRefresh: Readable<boolean> = writable(false)
  let updateServiceWorker: (reload?: boolean) => void = () => {}
  if (browser) {
    const real = useRegisterSW(swOptions)
    needRefresh = real.needRefresh
    updateServiceWorker = real.updateServiceWorker
  }

  function applyLoadedDocument(doc: SavedDocument) {
    markdown = doc.content
    imageAssets = (doc.assets ?? {}) as Record<string, LocalImageAsset>
    documentStore.finishDocumentTransition()
  }

  $effect(() => {
    if (hasInitializedMarkdown) return
    if (!browser) return
    hasInitializedMarkdown = true
    ;(async () => {
      const hasSeedMarkdown = initialMarkdown.trim() !== ''
      await documentStore.init({ restoreCurrent: !hasSeedMarkdown })
      if (hasSeedMarkdown) {
        documentStore.setCurrentDocument(null, false)
        markdown = initialMarkdown
        return
      }
      const invalidAutoDocs = documentStore.recentDocuments.filter(
        (doc) => isLegacyImplicitBlankDocument(doc) || isBrokenTemplateDocument(doc),
      )
      if (invalidAutoDocs.length > 0) {
        for (const doc of invalidAutoDocs) {
          await documentStore.deleteDocument(doc.id)
        }
      }
      const usableDocs = documentStore.recentDocuments.filter(
        (doc) => !isLegacyImplicitBlankDocument(doc) && !isBrokenTemplateDocument(doc),
      )
      if (documentStore.currentDocId) {
        const currentDoc = documentStore.recentDocuments.find(
          (d) => d.id === documentStore.currentDocId,
        )
        if (currentDoc && !isLegacyImplicitBlankDocument(currentDoc)) {
          const doc = await documentStore.loadDocument(documentStore.currentDocId)
          if (doc !== null) {
            applyLoadedDocument(doc)
            return
          }
        }
      }
      // Check if there are recent PDF docs
      if (usableDocs.length > 0) {
        const doc = await documentStore.loadDocument(usableDocs[0].id)
        if (doc !== null) {
          applyLoadedDocument(doc)
          return
        }
      }
      // Create new doc from initial markdown or template
      const defaultContent = initialMarkdown || PDF_TEMPLATES[0]?.content || ''
      markdown = defaultContent
      const doc = await documentStore.createDocument(defaultContent, undefined, 'template')
      applyLoadedDocument(doc)
    })()
  })

  // Layout state
  let leftPaneWidth = $state(50)
  let isResizing = $state(false)
  let isDragging = $state(false)

  // Which panes are on screen. Mirrored into the URL as `?edit` / `?view` /
  // `?both`, so a link can open the app the way the sender had it.
  type ViewMode = 'edit' | 'view' | 'both'
  const VIEW_MODES: ViewMode[] = ['edit', 'both', 'view']
  let viewMode = $state<ViewMode>('both')
  let showEditor = $derived(viewMode !== 'view')
  let showPreview = $derived(viewMode !== 'edit')

  function setViewMode(next: ViewMode) {
    viewMode = next
    // View mode has no paged preview: a page is a PDF idea.
    if (next === 'view') previewMode = 'document'
    const url = new URL(page.url)
    for (const mode of VIEW_MODES) url.searchParams.delete(mode)
    // `?view` reads better than `?view=`, and both parse the same. Built by
    // hand rather than by patching what `URLSearchParams` serialised: with a
    // fragment present the `=` is followed by `#`, so a trailing-only fixup
    // left `/?view=#heading` behind.
    const params = [...url.searchParams].map(([k, v]) => (v === '' ? k : `${k}=${v}`))
    url.search = [...params, next].join('&')
    replaceState(url.href, page.state)
  }

  let htmlPreview = $state<HtmlPreview | null>(null)
  let documentMenu = $state<DocumentMenu | null>(null)
  let isShortcutsOpen = $state(false)

  /** Whether a key event came from somewhere text is being entered. */
  function isTyping(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null
    if (!el?.tagName) return false
    return (
      el.isContentEditable ||
      ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) ||
      !!el.closest?.('.cm-editor')
    )
  }

  /**
   * Follow an outline jump in the URL. Inside a shadow root the browser cannot
   * resolve a fragment itself, so it neither scrolls nor records where the
   * reader is — which left `?view` unable to produce a link to a section.
   */
  function setHash(id: string) {
    const url = new URL(page.url)
    url.hash = id
    replaceState(url.href, page.state)
  }

  // Scroll sync, for the Web preview only — the paged preview is a Typst
  // render with nothing tying a page back to a line.
  //
  // Each side drives the other, so a flag suppresses the echo. It clears on
  // the next frame rather than after a timeout, because that is exactly how
  // long the scroll it caused takes to arrive.
  let syncing = false

  function sync(move: () => void) {
    if (syncing || previewMode !== 'document' || !showEditor || !showPreview) return
    syncing = true
    move()
    requestAnimationFrame(() => {
      syncing = false
    })
  }

  // A `#heading` the page was opened with, applied once the document it names
  // has actually been rendered.
  let pendingHash = ''

  $effect(() => {
    if (!browser || !pendingHash || !htmlDoc) return
    if (htmlPreview?.scrollTo(pendingHash)) pendingHash = ''
  })

  // Mobile state
  let activeMobileTab = $state<'editor' | 'preview'>('editor')
  let isAboutOpen = $state(false)
  let isExportOpen = $state(false)

  // Compilation state
  let status: 'idle' | 'compiling' | 'done' | 'error' = $state('idle')
  let errorMessage: string | null = $state(null)
  // Loading state
  let isLoading = $state(true)
  let loadingText = $state('Initializing...')

  // Typst client
  let client = $state<TypstWorkerClient | null>(null)
  let previewContainerEl = $state<HTMLDivElement | null>(null)
  let showPreviewCompilingHint = $state(false)
  let compilingHintTimer: number | null = null

  // SVG preview state
  let previewDoc = $state<SvgDocument | null>(null)
  let svgContainerEl = $state<HTMLDivElement | null>(null)
  let svgPageCount = $state(0)
  let svgScale = $state(1)

  // Pageless HTML view. The engine renders it without a Typst compile, so it
  // updates on its own short debounce instead of the adaptive compile one.
  type PreviewMode = 'pages' | 'document'
  let previewMode = $state<PreviewMode>('pages')
  let htmlDoc = $state('')
  // Content the render had to degrade — an image it could not fetch, a diagram
  // it could not draw. The document still renders, which is why these would
  // otherwise pass unnoticed.
  let warnings = $state<string[]>([])
  let isWarningsOpen = $state(false)
  const unreachable = (url: string) => `could not fetch: ${url}`
  let htmlTimer: number | null = null
  const HTML_DEBOUNCE_MS = 120

  // Auto-compile
  let compileSeq = 0
  let hasEverCompiled = false

  // Cached last compiled Markdown + images for PDF export (same as preview)
  // What the paged preview last compiled, so a remote image arriving late can
  // tell whether the document has moved on since.
  let compiledMarkdown = ''
  let autoPreviewTimer: number | null = null
  // The auto-compile debounce follows the last compile's duration, so a slow
  // document is not re-queued faster than it can be rendered.
  let lastCycleMs = 0

  $effect(() => {
    if (!browser) return

    if (compilingHintTimer !== null) {
      window.clearTimeout(compilingHintTimer)
      compilingHintTimer = null
    }

    showPreviewCompilingHint = false

    if (status === 'compiling' && !!previewDoc) {
      compilingHintTimer = window.setTimeout(() => {
        showPreviewCompilingHint = true
      }, 180)
    }

    return () => {
      if (compilingHintTimer !== null) {
        window.clearTimeout(compilingHintTimer)
        compilingHintTimer = null
      }
    }
  })

  onMount(() => {
    loadingText = 'Initializing rendering engine...'
    client = getSharedTypstWorkerClient()

    // A mode in the URL wins over the default split.
    const requested = VIEW_MODES.find((mode) => page.url.searchParams.has(mode))
    if (requested) {
      viewMode = requested
      if (requested === 'view') {
        previewMode = 'document'
        activeMobileTab = 'preview'
      }
    }
    // `/?view#some-heading` is a link to a section, so honour it once the
    // document naming that heading has rendered.
    if (page.url.hash.length > 1) {
      pendingHash = decodeURIComponent(page.url.hash.slice(1))
      previewMode = 'document'
    }

    // Hide loading overlay and trigger first compile
    isLoading = false
    void compile(markdown)

    // Close menus on click outside. Guarded so the global click listener
    // doesn't reactively touch state on every click in the editor — Svelte
    // 5 already short-circuits identical writes, but skipping the call
    // entirely keeps this listener off the hot path.
    const handleClickOutside = () => {
      if (isExportOpen) isExportOpen = false
    }
    window.addEventListener('click', handleClickOutside)

    // Capture phase + stopImmediatePropagation, so CodeMirror's own keymap
    // never sees a shortcut we have claimed and cannot, say, insert a newline
    // for Ctrl+Enter as well as compiling.
    const handleKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey
      const claim = () => {
        e.preventDefault()
        e.stopImmediatePropagation()
        e.stopPropagation()
      }

      if (e.key === 'Escape') {
        if (!isShortcutsOpen && !isAboutOpen && !isExportOpen && !replacePrompt) return
        claim()
        isShortcutsOpen = false
        isAboutOpen = false
        isExportOpen = false
        replacePrompt?.answer(false)
        return
      }
      // `?` where you are not typing — the usual way to ask for this list.
      if (!mod && e.key === '?' && !isTyping(e.target)) {
        claim()
        isShortcutsOpen = !isShortcutsOpen
        return
      }
      if (!mod) return

      switch (e.key.toLowerCase()) {
        case 'enter':
          claim()
          compileNow()
          return
        case 'n':
          // Cmd/Ctrl+N belongs to the browser, so this one takes Alt too.
          if (!e.altKey || readOnly) return
          claim()
          void documentMenu?.newBlank()
          return
        case 's':
          // Swallowed rather than handled: there is nothing to save, and the
          // browser's "save page as" is not what anyone means by it here.
          claim()
          return
        case 'e':
          claim()
          isExportOpen = !isExportOpen
          return
        case 'p':
          claim()
          void downloadPdf()
          return
        case '1':
        case '2':
        case '3':
          claim()
          setViewMode(VIEW_MODES[Number(e.key) - 1])
          return
        case '\\':
          claim()
          if (viewMode !== 'view') previewMode = previewMode === 'pages' ? 'document' : 'pages'
          return
        case 'o':
          claim()
          previewMode = 'document'
          htmlPreview?.toggleOutline()
          return
        case '/':
          claim()
          isShortcutsOpen = !isShortcutsOpen
          return
      }
    }
    window.addEventListener('keydown', handleKey, true)

    // Debounced resize handler for auto-fit
    let resizeTimer: number | null = null
    const handleResize = () => {
      if (resizeTimer) clearTimeout(resizeTimer)
      resizeTimer = window.setTimeout(() => {
        fitWidth()
      }, 200)
    }
    window.addEventListener('resize', handleResize)

    // Typing is buffered and autosave debounced — both must land before the
    // tab goes away.
    const handleHide = () => {
      editorPane?.flushPendingEdit()
      void documentStore.flushPendingSave()
    }
    document.addEventListener('visibilitychange', handleHide)
    window.addEventListener('pagehide', handleHide)

    return () => {
      window.removeEventListener('click', handleClickOutside)
      window.removeEventListener('resize', handleResize)
      document.removeEventListener('visibilitychange', handleHide)
      window.removeEventListener('pagehide', handleHide)
      window.removeEventListener('keydown', handleKey, true)
      if (resizeTimer) clearTimeout(resizeTimer)
      if (swUpdateTimer) clearInterval(swUpdateTimer)
    }
  })

  // Auto-save document to IndexedDB. Images are written only when they change:
  // a save without them keeps the stored ones, so ordinary typing does not
  // re-serialize every image in the document on each keystroke.
  let savedAssets: Record<string, SavedDocumentAsset> | null = null
  $effect(() => {
    if (!browser || !hasInitializedMarkdown || !documentStore.currentDocId) return
    if (documentStore.isTransitioningDocument) return
    const assets = documentAssets
    const changed = assets !== savedAssets
    savedAssets = assets
    documentStore.autoSave(documentStore.currentDocId, markdown, changed ? assets : undefined)
  })

  // Auto-compile effect. Only while the paged preview is actually on screen:
  // a Typst compile is the most expensive thing the app does, and on the Web
  // tab — or in `?view`, which is always the Web tab — its result is never
  // looked at. Export compiles on demand instead of relying on this.
  // Important: subscribe to `markdown` ONLY when we'll actually use it, so a
  // keystroke behind a hidden pane does not re-run this effect for nothing.
  $effect(() => {
    if (!browser) return
    if (!client) return
    if (isLoading) return
    if (!showPreview || previewMode !== 'pages') return

    const md = markdown
    // Switching back to Pages with nothing changed since: the pages on screen
    // are already the answer.
    if (previewDoc && md === compiledMarkdown) return

    if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)

    const delay = hasEverCompiled ? Math.min(Math.max(450, lastCycleMs), 2500) : 0
    autoPreviewTimer = window.setTimeout(() => {
      void compile(md)
    }, delay)

    return () => {
      if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)
    }
  })

  function compileNow() {
    // Compile what is on screen, including keystrokes still buffered.
    editorPane?.flushPendingEdit()
    if (previewMode === 'pages') void compile(markdown)
    else void renderHtml(markdown)
  }

  // The HTML view costs a few milliseconds, so it tracks typing closely rather
  // than waiting for the Typst compile. Only runs while its tab is showing.
  $effect(() => {
    if (!browser) return
    if (!client) return
    if (isLoading) return
    if (previewMode !== 'document') return

    const md = markdown
    if (htmlTimer) window.clearTimeout(htmlTimer)
    htmlTimer = window.setTimeout(() => void renderHtml(md), HTML_DEBOUNCE_MS)

    return () => {
      if (htmlTimer) window.clearTimeout(htmlTimer)
    }
  })

  let htmlSeq = 0

  async function renderHtml(md: string) {
    if (!client) return
    const seq = ++htmlSeq
    try {
      // `editable` gives the fragment its source lines and live checkboxes;
      // the reference view has a source it will not write to, so it opts out.
      const { html, diagnostics } = await client.renderHtml(
        md,
        imagesToSend(documentImages(md)),
        false,
        !readOnly,
      )
      // Both guards matter: `seq` catches a render this component superseded,
      // `md === markdown` catches one that finished against text the document
      // has since moved past — which is how a checkbox click during the
      // debounce window got repainted in its old state.
      if (seq === htmlSeq && md === markdown) {
        htmlDoc = html
        warnings = [...new Set([...diagnostics, ...cachedRemoteImages(md).failed.map(unreachable)])]
      }
    } catch (error) {
      // A render the worker dropped in favour of a newer one is not an error.
      if (error instanceof Error && error.message === SUPERSEDED) return
      if (seq !== htmlSeq) return
      status = 'error'
      errorMessage = error instanceof Error ? error.message : String(error)
    }
  }

  // Auto-fit on mobile tab switch, once the pane has been laid out.
  $effect(() => {
    if (!browser) return
    if (activeMobileTab !== 'preview') return
    const timer = window.setTimeout(fitWidth, 50)
    return () => window.clearTimeout(timer)
  })

  let slots: ReturnType<typeof pageSlots> | null = null

  $effect(() => {
    if (!browser || !svgContainerEl) return
    slots = pageSlots(svgContainerEl)
    return () => {
      slots?.destroy()
      slots = null
    }
  })

  $effect(() => {
    if (!previewDoc || !slots) return
    svgPageCount = previewDoc.pages.length
    slots.show(previewDoc)
  })

  // All Markdown processing happens inside the Typst compile (the md2pdf
  // engine). The host only resolves images Typst cannot fetch itself.
  async function compile(md: string) {
    if (!client) return
    // Only mark "compiled" once real content exists, so the adaptive delay
    // below is not calibrated against an empty first pass.
    if (md.trim() !== '') hasEverCompiled = true

    const seq = ++compileSeq
    // Whatever is still queued in the worker is already stale.
    client.cancelPendingPreview()
    status = 'compiling'
    errorMessage = null
    const startedAt = performance.now()

    try {
      // Remote images are not awaited: compile with what is cached, then
      // recompile if a fetch brings in something new.
      const { missing } = cachedRemoteImages(md)
      const images = documentImages(md)

      compiledMarkdown = md

      if (missing.length > 0) {
        void prefetchRemoteImages(missing).then((gotNew) => {
          if (!gotNew || compiledMarkdown !== md) return
          void compile(md)
          if (previewMode === 'document') void renderHtml(md)
        })
      }

      const result = await client.compilePreview(md, imagesToSend(images))
      if (seq !== compileSeq) return
      previewDoc = result.preview
      status = 'done'
      lastCycleMs = performance.now() - startedAt
    } catch (error) {
      // A compile the worker dropped in favour of a newer one is not an error.
      if (error instanceof Error && error.message === SUPERSEDED) return
      if (seq !== compileSeq) return
      status = 'error'
      errorMessage = error instanceof Error ? error.message : String(error)
    }
  }

  /** Every image byte-array the document references, local and remote. */
  function documentImages(md: string): ImageMap {
    return { ...collectReferencedImageAssets(md), ...cachedRemoteImages(md).images }
  }

  // The worker keeps every image it has been handed, so a recompile only
  // ships new ones instead of copying megabytes across the worker boundary.
  const sentImages = new Map<string, Uint8Array<ArrayBuffer>>()

  function imagesToSend(images: ImageMap): ImageMap {
    const fresh: ImageMap = {}
    for (const [path, bytes] of Object.entries(images)) {
      if (sentImages.get(path) === bytes) continue
      sentImages.set(path, bytes)
      fresh[path] = bytes
    }
    return fresh
  }

  async function downloadPdf() {
    editorPane?.flushPendingEdit()
    if (!client) return
    // Compiled from what is on screen rather than from whatever the preview
    // last rendered — the paged preview may never have run at all.
    const md = markdown
    // Open the tab synchronously so it counts as user-initiated and isn't
    // blocked by popup blockers after the async compile.
    const newTab = window.open('', '_blank')
    // @ts-ignore
    const { pdf } = await client.compilePdf(md, imagesToSend(documentImages(md)))
    const blob = new Blob([pdf], { type: 'application/pdf' })
    const url = URL.createObjectURL(blob)
    if (newTab) {
      newTab.location.href = url
    } else {
      window.open(url, '_blank')
    }
    setTimeout(() => URL.revokeObjectURL(url), 60_000)
  }

  function download(blob: Blob, extension: string) {
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download =
      (deriveNameFromContent(markdown) || 'document').replace(/[/\\?%*:|"<>]/g, '-') + extension
    link.click()
    setTimeout(() => URL.revokeObjectURL(url), 60_000)
  }

  async function downloadHtml() {
    editorPane?.flushPendingEdit()
    if (!client) return
    const md = markdown
    const { html } = await client.renderHtml(md, imagesToSend(documentImages(md)), true)
    download(new Blob([html], { type: 'text/html;charset=utf-8' }), '.html')
  }

  function downloadMarkdown() {
    editorPane?.flushPendingEdit()
    download(new Blob([markdown], { type: 'text/markdown;charset=utf-8' }), '.md')
  }

  let fileInputEl = $state<HTMLInputElement | null>(null)

  function handleUpload() {
    fileInputEl?.click()
  }

  function onFileSelected(e: Event) {
    const target = e.target as HTMLInputElement
    const file = target.files?.[0]
    // Reset value so the same file can be picked again
    target.value = ''
    if (file) void openFile(file)
  }

  // `confirm()` blocks the whole tab, which stops the worker's messages from
  // being delivered and leaves the preview frozen behind the dialog.
  let replacePrompt = $state<{ name: string; answer: (ok: boolean) => void } | null>(null)

  function confirmReplace(name: string): Promise<boolean> {
    return new Promise((resolve) => {
      replacePrompt = {
        name,
        answer: (ok) => {
          replacePrompt = null
          resolve(ok)
        },
      }
    })
  }

  /** An image is inserted where the cursor is; anything else replaces the document. */
  async function openFile(file: File) {
    if (file.type.startsWith('image/')) {
      await editorPane?.insertImageFile(file)
      return
    }
    if (markdown.trim() !== '' && !(await confirmReplace(file.name))) return
    markdown = await file.text()
  }

  function handleImageSaved(path: string, bytes: Uint8Array<ArrayBuffer>, mimeType: string) {
    imageAssets[path] = { bytes, mimeType }
  }

  function collectReferencedImageAssets(md: string): ImageMap {
    const referenced = new Set<string>()
    // The tail also covers HackMD `=WxH` sizing, not just a quoted title.
    const markdownImageRegex = /!\[[^\]]*]\(([^)\s]+)(?:\s+[^)]*)?\)/g

    for (const match of md.matchAll(markdownImageRegex)) {
      const path = match[1]
      if (path in imageAssets) {
        referenced.add(path)
      }
    }

    return Object.fromEntries([...referenced].map((path) => [path, imageAssets[path].bytes]))
  }

  function startResize(e: MouseEvent) {
    e.preventDefault()
    isResizing = true
    document.addEventListener('mousemove', onResize)
    document.addEventListener('mouseup', stopResize)
  }

  // A width write relayouts both panes, so collapse the mousemove stream to
  // one write per frame.
  let resizeRaf: number | null = null
  let pendingPaneWidth = 50

  function onResize(e: MouseEvent) {
    if (!isResizing) return
    pendingPaneWidth = Math.min(Math.max((e.clientX / window.innerWidth) * 100, 20), 80)
    if (resizeRaf !== null) return
    resizeRaf = requestAnimationFrame(() => {
      resizeRaf = null
      leftPaneWidth = pendingPaneWidth
    })
  }

  function stopResize() {
    isResizing = false
    if (resizeRaf !== null) {
      cancelAnimationFrame(resizeRaf)
      resizeRaf = null
      leftPaneWidth = pendingPaneWidth
    }
    document.removeEventListener('mousemove', onResize)
    document.removeEventListener('mouseup', stopResize)
  }

  // A `separator` with `tabindex` that answers to no key is worse than one
  // that cannot be focused at all — it is a stop on the tab order that does
  // nothing. Home/End go to the two ends of the allowed range.
  function resizeKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 2
    const to = {
      ArrowLeft: leftPaneWidth - step,
      ArrowRight: leftPaneWidth + step,
      Home: 20,
      End: 80,
    }[e.key]
    if (to === undefined) return
    e.preventDefault()
    leftPaneWidth = Math.min(Math.max(to, 20), 80)
  }

  function hasFiles(e: DragEvent): boolean {
    return e.dataTransfer?.types?.includes('Files') ?? false
  }

  function handleDragOver(e: DragEvent) {
    if (!hasFiles(e) || readOnly) return
    e.preventDefault()
    isDragging = true
  }

  function handleDragLeave(e: DragEvent) {
    if (!hasFiles(e)) return
    e.preventDefault()
    isDragging = false
  }

  function handleDrop(e: DragEvent) {
    if (!hasFiles(e)) return
    e.preventDefault()
    isDragging = false
    if (readOnly) return

    const files = e.dataTransfer?.files
    if (!files || files.length === 0) return

    // Same two paths as the Upload button, prompt included.
    const file = getMarkdownImportFile(files) ?? getImageDropFile(files)
    if (file) void openFile(file)
  }

  async function handlePaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items
    if (!items || readOnly) return

    for (const item of items) {
      if (!item.type.startsWith('image/')) continue
      const file = item.getAsFile()
      if (!file) continue
      e.preventDefault()
      await editorPane?.insertImageFile(file)
      return
    }
  }

  function fitWidth() {
    svgScale = 1
  }

  function svgZoomIn() {
    svgScale = Math.min(svgScale + 0.25, 3)
  }

  function svgZoomOut() {
    svgScale = Math.max(svgScale - 0.25, 0.25)
  }

  // ResizeObserver for auto-fit
  $effect(() => {
    if (!previewContainerEl || !browser) return
    const observer = new ResizeObserver(() => {
      fitWidth()
    })
    observer.observe(previewContainerEl)
    return () => observer.disconnect()
  })
</script>

<svelte:head>
  <title>{seoTitle}</title>
  <meta name="description" content={seoDescription} />

  <!-- Open Graph — `summary`-style: small square thumbnail next to text,
       not a full-width hero image. -->
  <meta property="og:title" content={seoTitle} />
  <meta property="og:description" content={seoDescription} />
  <meta property="og:type" content="website" />
  <meta property="og:locale" content="en_US" />
  <meta property="og:image" content={SOCIAL_IMAGE} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="240" />
  <meta property="og:image:height" content="240" />
  <meta property="og:image:alt" content="md2pdf logo" />

  <!-- Twitter / Discord embed (summary keeps the icon as a side thumbnail) -->
  <meta name="twitter:card" content="summary" />
  <meta name="twitter:title" content={seoTitle} />
  <meta name="twitter:description" content={seoDescription} />
  <meta name="twitter:image" content={SOCIAL_IMAGE} />
</svelte:head>

<!-- Loading Overlay -->
<div class="loading-overlay" class:hidden={!isLoading}>
  <div class="loading-spinner"></div>
  <div class="loading-progress">
    <div class="loading-progress-bar"></div>
  </div>
  <div class="loading-text">{loadingText}</div>
</div>

<!-- Main App -->
<div
  class="app"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onpaste={handlePaste}
  role="application"
>
  {#if isDragging}
    <div class="drop-overlay">
      <div class="drop-overlay-content">
        <svg
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="17 8 12 3 7 8"></polyline>
          <line x1="12" y1="3" x2="12" y2="15"></line>
        </svg>
        <span>Drop .md or image files here</span>
      </div>
    </div>
  {/if}

  <!-- File Input (Hidden) -->
  <input
    type="file"
    accept="image/png,image/jpeg,image/webp,image/svg+xml,image/gif,.md,.markdown,.txt"
    style="display: none;"
    bind:this={fileInputEl}
    onchange={onFileSelected}
  />

  <!-- Navbar -->
  <nav class="navbar">
    <div class="navbar-left">
      <a href="/" class="logo-link">
        <img src="/logo.png" alt="md2pdf" class="logo-img" />
      </a>
      {#if readOnly}
        <span class="doc-title">Reference</span>
      {:else}
        <DocumentMenu
          bind:this={documentMenu}
          currentContent={markdown}
          {documentAssets}
          onDocumentLoad={(doc) => {
            applyLoadedDocument(doc)
          }}
        />
      {/if}
    </div>

    <!-- Centred, so a long document name never shifts it. -->
    <div class="view-switch layout-switch" role="group" aria-label="Layout">
      {#each VIEW_MODES as mode}
        <button
          class:active={viewMode === mode}
          aria-pressed={viewMode === mode}
          onclick={() => setViewMode(mode)}
          title={mode === 'edit' ? 'Editor only' : mode === 'view' ? 'Document only' : 'Split'}
        >
          <!-- Which half is filled is which pane you get. -->
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
          >
            {#if mode !== 'view'}
              <rect x="3" y="5" width="9" height="14" fill="currentColor" stroke="none"></rect>
            {/if}
            {#if mode !== 'edit'}
              <rect x="12" y="5" width="9" height="14" fill="currentColor" stroke="none"></rect>
            {/if}
            <rect x="3" y="5" width="18" height="14" rx="2"></rect>
            <path d="M12 5v14"></path>
          </svg>
        </button>
      {/each}
    </div>

    <div class="navbar-right">
      {#if $needRefresh}
        <button class="tool-btn update-btn" onclick={() => updateServiceWorker(true)}>
          Update available
        </button>
      {/if}
      {@render themeToggle()}

      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="menu-container" onclick={(e) => e.stopPropagation()}>
        <button
          class="btn btn-sm export-btn"
          onclick={() => (isExportOpen = !isExportOpen)}
          aria-expanded={isExportOpen}
        >
          Export
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path
              d="M3 4.5L6 7.5L9 4.5"
              stroke="currentColor"
              stroke-width="1.5"
              fill="none"
              stroke-linecap="round"
            />
          </svg>
        </button>
        {#if isExportOpen}
          <div class="dropdown-menu">
            <button
              class="menu-item"
              onclick={() => {
                isExportOpen = false
                void downloadPdf()
              }}
              disabled={!previewDoc}>PDF</button
            >
            <button
              class="menu-item"
              onclick={() => {
                isExportOpen = false
                downloadMarkdown()
              }}>Markdown</button
            >
            <button
              class="menu-item"
              onclick={() => {
                isExportOpen = false
                void downloadHtml()
              }}>HTML</button
            >
          </div>
        {/if}
      </div>
    </div>
  </nav>

  {#snippet themeToggle()}
    <button
      class="tool-btn tool-btn-icon"
      onclick={() => settingsStore.setTheme(settingsStore.theme === 'dark' ? 'light' : 'dark')}
      title={settingsStore.theme === 'dark' ? 'Switch to light' : 'Switch to dark'}
      aria-label={settingsStore.theme === 'dark' ? 'Switch to light' : 'Switch to dark'}
    >
      {#if settingsStore.theme === 'dark'}
        <svg
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="4"></circle>
          <path
            d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
          ></path>
        </svg>
      {:else}
        <svg
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"></path>
        </svg>
      {/if}
    </button>
  {/snippet}

  <!-- The document tools, in the strip above the editor. -->
  {#snippet tools()}
    <div class="toolbar">
      {#if !readOnly}
        <button
          class="tool-btn"
          onclick={() => editorPane?.insertMarkdownSnippet(`\n\n${PAGEBREAK_TOKEN}\n\n`)}
          title="Insert page break"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="2" y1="12" x2="6" y2="12"></line>
            <line x1="18" y1="12" x2="22" y2="12"></line>
            <path d="M6 8V4h12v4"></path>
            <path d="M6 16v4h12v-4"></path>
          </svg>
          <span class="tool-label">Break</span>
        </button>
        <button
          class="tool-btn"
          onclick={handleUpload}
          title="Insert an image, or open a Markdown file"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="17 8 12 3 7 8"></polyline>
            <line x1="12" y1="3" x2="12" y2="15"></line>
          </svg>
          <span class="tool-label">Upload</span>
        </button>
      {/if}

      {@render themeToggle()}

      {#if !readOnly}
        <a
          class="tool-btn tool-btn-icon"
          href="/reference"
          target="_blank"
          rel="noopener"
          title="Reference — every feature, source next to result"
          aria-label="Reference">?</a
        >
      {/if}

      <button
        class="tool-btn tool-btn-icon"
        onclick={() => (isShortcutsOpen = true)}
        title="Keyboard shortcuts (?)"
        aria-label="Keyboard shortcuts"
      >
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect
            x="0.75"
            y="3.25"
            width="14.5"
            height="9.5"
            rx="1.75"
            stroke="currentColor"
            stroke-width="1.3"
          />
          <path
            d="M4 6h.01M6.5 6h.01M9 6h.01M11.5 6h.01M4.5 9.5h6"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
          />
        </svg>
      </button>

      <button
        class="tool-btn tool-btn-icon"
        onclick={() => (isAboutOpen = true)}
        title="About md2pdf"
        aria-label="About md2pdf"
      >
        <svg class="gh-icon" width="15" height="15" viewBox="0 0 16 16" aria-hidden="true">
          <path
            d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.4 7.4 0 0 1 2-.27c.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z"
          />
        </svg>
      </button>
    </div>
  {/snippet}

  <!-- Workspace -->
  <main class="workspace">
    <!-- Editor Pane -->
    <section
      class="pane editor-pane"
      class:hidden={!showEditor}
      class:mobile-hidden={activeMobileTab !== 'editor'}
      style="width: {showPreview ? leftPaneWidth : 100}%"
    >
      {@render tools()}
      <EditorPane
        bind:this={editorPane}
        bind:markdown
        placeholder="Type Markdown here..."
        {errorMessage}
        {readOnly}
        onImageSaved={handleImageSaved}
        onNewDocument={() => void documentMenu?.newBlank()}
        onScrolled={() =>
          sync(() => {
            const line = editorPane?.topLine()
            if (line) htmlPreview?.scrollToLine(line)
          })}
      />
    </section>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="resizer hidden-mobile"
      class:hidden={viewMode !== 'both'}
      class:active={isResizing}
      onmousedown={startResize}
      onkeydown={resizeKey}
      role="separator"
      aria-label="Editor width"
      aria-orientation="vertical"
      aria-valuemin={20}
      aria-valuemax={80}
      aria-valuenow={Math.round(leftPaneWidth)}
      tabindex="0"
    ></div>

    <!-- Mobile Tab Switcher (Visible only on mobile) -->
    <div class="mobile-tabs">
      <button
        class="mobile-tab-btn"
        class:active={activeMobileTab === 'editor'}
        onclick={() => (activeMobileTab = 'editor')}
      >
        Editor
      </button>
      <button
        class="mobile-tab-btn"
        class:active={activeMobileTab === 'preview'}
        onclick={() => (activeMobileTab = 'preview')}
      >
        Preview
      </button>
    </div>

    <!-- Preview Pane -->
    <section
      class="pane preview-pane"
      class:hidden={!showPreview}
      class:mobile-hidden={activeMobileTab !== 'preview'}
    >
      <!-- View mode leaves nothing for this bar to hold, so it goes away. -->
      {#if viewMode !== 'view' || status === 'error'}
        <div class="preview-toolbar">
          <div class="preview-status-wrapper">
            <!-- View mode is for reading: no editing tools, no paged/pageless
               switch, just the document. -->
            {#if viewMode !== 'view'}
              <div class="view-switch" role="group" aria-label="Preview mode">
                <button
                  class:active={previewMode === 'pages'}
                  aria-pressed={previewMode === 'pages'}
                  onclick={() => (previewMode = 'pages')}
                >
                  Pages
                </button>
                <button
                  class:active={previewMode === 'document'}
                  aria-pressed={previewMode === 'document'}
                  onclick={() => {
                    previewMode = 'document'
                    if (!htmlDoc) void renderHtml(markdown)
                  }}
                >
                  Web
                </button>
              </div>
            {/if}
            {#if previewMode === 'pages'}
              <span class="page-info"
                >{svgPageCount || '—'} {svgPageCount === 1 ? 'page' : 'pages'}</span
              >
            {/if}
            {#if status === 'error'}
              <div class="error-badge">
                <span>⚠️ Failed</span>
              </div>
            {:else if warnings.length > 0}
              <button
                class="warning-badge"
                onclick={() => (isWarningsOpen = !isWarningsOpen)}
                aria-expanded={isWarningsOpen}
                title="Content the preview could not render"
              >
                {warnings.length}
                {warnings.length === 1 ? 'warning' : 'warnings'}
              </button>
            {/if}
          </div>
          <div class="preview-toolbar-right">
            {#if previewMode === 'pages'}
              <div class="zoom">
                <button onclick={svgZoomOut} disabled={svgScale <= 0.25}>-</button>
                <span class="zoom-level">{Math.round(svgScale * 100)}%</span>
                <button onclick={svgZoomIn} disabled={svgScale >= 3}>+</button>
                <button onclick={fitWidth} disabled={!previewDoc}>Fit</button>
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <div class="preview-container" bind:this={previewContainerEl}>
        {#if showPreviewCompilingHint}
          <StatusHint label="Updating preview" />
        {/if}
        <div
          class="svg-preview-container"
          class:hidden={previewMode !== 'pages'}
          style="--svg-scale: {svgScale}"
          bind:this={svgContainerEl}
        ></div>
        {#if previewMode === 'document'}
          {#if isWarningsOpen && warnings.length > 0}
            <ul class="warning-list">
              {#each warnings as warning (warning)}
                <li>{warning}</li>
              {/each}
            </ul>
          {/if}
          <div class="html-preview-container">
            <HtmlPreview
              bind:this={htmlPreview}
              html={htmlDoc}
              theme={settingsStore.theme}
              onnavigate={setHash}
              ontasktoggle={(line, checked) => editorPane?.setTaskMarker(line, checked)}
              onscrolled={() =>
                sync(() => {
                  const line = htmlPreview?.lineAtTop()
                  if (line) editorPane?.scrollToLine(line)
                })}
            />
          </div>
        {/if}
        {#if previewMode === 'pages' && status === 'compiling' && !previewDoc}
          <div class="preview-placeholder">
            <div class="loading-spinner"></div>
          </div>
        {/if}
      </div>
    </section>
  </main>

  {#if isShortcutsOpen}
    <ShortcutOverlay onClose={() => (isShortcutsOpen = false)} />
  {/if}

  {#if isAboutOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-backdrop" onclick={() => (isAboutOpen = false)}>
      <div
        class="modal-dialog"
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        tabindex="-1"
      >
        <h3 class="modal-title" id="about-title">About md2pdf</h3>
        <p class="modal-help">
          Markdown in, typeset PDF or a self-contained HTML page out. Everything — the Markdown
          engine, the Typst typesetter, the fonts — runs in this browser tab: no account, no upload,
          and it keeps working offline.
        </p>
        <p class="modal-help">
          Your documents live in this browser's storage only. <strong>Export</strong> is how you get them
          out.
        </p>
        <div class="modal-actions">
          <a
            class="btn btn-secondary btn-sm"
            href="https://github.com/libnewton/markdown2pdf"
            target="_blank"
            rel="noopener noreferrer">Source on GitHub</a
          >
          <button class="btn btn-primary btn-sm" onclick={() => (isAboutOpen = false)}>Close</button
          >
        </div>
      </div>
    </div>
  {/if}

  {#if replacePrompt}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-backdrop" onclick={() => replacePrompt?.answer(false)}>
      <div
        class="modal-dialog"
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="replace-title"
        tabindex="-1"
      >
        <h3 class="modal-title" id="replace-title">Replace this document?</h3>
        <p class="modal-help">
          Opening <strong>{replacePrompt.name}</strong> discards what is in the editor now.
        </p>
        <div class="modal-actions">
          <button class="btn btn-secondary btn-sm" onclick={() => replacePrompt?.answer(false)}>
            Cancel
          </button>
          <button class="btn btn-primary btn-sm" onclick={() => replacePrompt?.answer(true)}>
            Replace
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 999;
    background: light-dark(rgba(255, 255, 255, 0.85), rgba(20, 20, 20, 0.85));
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .drop-overlay-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--color-gray-500);
    font-size: 1rem;
    font-weight: 500;
    padding: 40px 60px;
    border: 2px dashed var(--color-gray-300);
    border-radius: 16px;
    background: var(--color-gray-50);
  }

  /* Three columns, so the middle one sits dead centre no matter how wide the
     document name in the first one gets. */
  .navbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    height: var(--navbar-height);
    padding: 0 var(--space-md);
    background: var(--color-white);
    border-bottom: 1px solid var(--color-gray-200);
    flex-shrink: 0;
  }

  .navbar-left,
  .navbar-right {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .navbar-left {
    flex: 1;
    min-width: 0;
  }

  .navbar-right {
    justify-content: flex-end;
    gap: var(--space-sm);
  }

  /* The document tools, in the strip above the editor. In view mode the same
     strip is rendered into the preview toolbar instead, so it goes wherever
     the content is. */
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
    flex-shrink: 0;
  }

  .editor-pane > .toolbar {
    height: var(--pane-toolbar-height);
    padding: 0 var(--space-sm);
    background: var(--color-gray-50);
    border-bottom: 1px solid var(--color-gray-200);
  }

  .logo-link {
    display: flex;
    align-items: center;
    height: 100%;
    text-decoration: none;
  }

  .logo-img {
    height: 28px;
    width: auto;
    display: block;
  }

  /* The wordmark is dark ink on transparent. Inverting and rotating the hue
     back keeps the mark's colour while the letters turn light. */
  :global([data-theme='dark']) .logo-img {
    filter: invert(1) hue-rotate(180deg);
  }

  .doc-title {
    padding: 4px 8px;
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--color-gray-700);
    white-space: nowrap;
  }

  /* The layout switch is its own grid column, so the document name beside it
     can grow to any length without pushing it around. */
  .layout-switch {
    justify-self: center;
  }

  /* Every control in the chrome is the same typeface, weight and size — only
     the height changes between the navbar row and the pane strips. */
  .navbar :is(.btn, .tool-btn, .view-switch button),
  .toolbar :is(.tool-btn, .view-switch button),
  .preview-toolbar :is(.tool-btn, .view-switch button, .zoom button, .page-info, .zoom-level) {
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 1;
  }

  /* Toolbar buttons: one shape, label optional. */
  .tool-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: var(--control);
    padding: 0 8px;
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    background: var(--color-white);
    color: var(--color-gray-600);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .tool-btn:hover {
    background: var(--color-gray-100);
    border-color: var(--color-gray-300);
    color: var(--color-gray-900);
  }

  .tool-btn-icon {
    width: var(--control);
    padding: 0;
  }

  .export-btn {
    gap: 4px;
    height: var(--control-lg);
    padding: 0 0.75rem;
    background: var(--color-gray-100);
    color: var(--color-gray-900);
    border: 1px solid var(--color-gray-300);
    box-shadow: var(--shadow-sm);
  }

  .export-btn:hover {
    background: var(--color-gray-200);
    border-color: var(--color-gray-400);
  }

  /* In the navbar the controls are one size up from the pane strips. */
  .navbar :is(.tool-btn, .view-switch) {
    height: var(--control-lg);
  }

  .navbar .tool-btn-icon {
    width: var(--control-lg);
  }

  /* The Typst preview's own SVG carries a stylesheet, and an inline `<svg>`
     stylesheet is document-wide — its `svg { fill: none }` outranks any fill
     *attribute*. The stroke icons don't care; a solid one has to say it here. */
  .gh-icon {
    fill: currentColor;
  }

  .update-btn {
    color: var(--color-success);
    border-color: var(--color-success);
  }

  @media (max-width: 1100px) {
    .tool-label {
      display: none;
    }
    .tool-btn {
      width: var(--control);
      padding: 0;
    }
  }

  .workspace {
    flex: 1;
    display: flex;
    overflow: hidden;
    background-color: var(--color-gray-100);
  }

  .pane {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    position: relative;
    background: var(--color-white);
  }

  /* A hidden pane stays mounted: CodeMirror keeps its state and the preview
     keeps its measured pages, so switching layout is a style recalc, not a
     rebuild. */
  .pane.hidden,
  .resizer.hidden {
    display: none;
  }

  /* Editor Pane */
  .editor-pane {
    background: var(--editor-bg);
    position: relative;
  }

  .resizer {
    width: var(--divider-width);
    background: var(--color-gray-200);
    cursor: col-resize;
    flex-shrink: 0;
    position: relative;
    transition: background var(--transition-fast);
  }

  .resizer::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 4px;
    height: 40px;
    background: repeating-linear-gradient(
      to bottom,
      var(--color-gray-400) 0px,
      var(--color-gray-400) 2px,
      transparent 2px,
      transparent 6px
    );
    border-radius: 2px;
    opacity: 0.5;
  }

  .resizer:hover,
  .resizer:focus-visible,
  .resizer.active {
    background: var(--color-gray-400);
  }

  .resizer:hover::after,
  .resizer:focus-visible::after,
  .resizer.active::after {
    opacity: 1;
  }

  /* Preview Pane. It takes whatever the editor and the divider leave, rather
     than a percentage of its own — three widths adding up to 100% plus a
     6px divider is 6px too many, and the overflow shows at the right edge. */
  .preview-pane {
    flex: 1 1 auto;
    min-width: 0;
    background: var(--preview-bg);
  }

  /* Same height as the editor's strip, so the two line up across the divider. */
  .preview-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--pane-toolbar-height);
    /* The right edge lines up with Export's, one bar above. */
    padding: 0 var(--space-md) 0 var(--space-sm);
    background: var(--color-gray-50);
    border-bottom: 1px solid var(--color-gray-200);
    flex-shrink: 0;
  }

  .preview-toolbar-right {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  /* Segmented controls: the paged/pageless switch and the layout switch. Both
     are built to the same height as a toolbar button. */
  .view-switch {
    display: flex;
    align-items: center;
    height: var(--control);
    background: var(--color-gray-100);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    padding: 2px;
    gap: 2px;
  }

  .view-switch button {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    min-width: 30px;
    padding: 0 10px;
    color: var(--color-gray-500);
    background: transparent;
    border: 0;
    border-radius: calc(var(--radius-sm) - 1px);
    cursor: pointer;
  }

  .view-switch button.active {
    background: var(--color-white);
    color: var(--color-gray-900);
    box-shadow: 0 1px 2px rgba(16, 24, 40, 0.08);
  }

  .view-switch button svg {
    display: block;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .zoom button {
    height: var(--control);
    min-width: var(--control);
    padding: 0 var(--space-sm);
    color: var(--color-gray-600);
    background: var(--color-white);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .zoom button:hover:not(:disabled) {
    background: var(--color-gray-100);
    border-color: var(--color-gray-300);
    color: var(--color-gray-900);
  }

  .zoom button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .page-info,
  .zoom-level {
    color: var(--color-gray-500);
    font-variant-numeric: tabular-nums;
  }

  .tool-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .preview-container {
    flex: 1;
    overflow: hidden;
    position: relative;
    /* Isolate the heavy SVG preview from layout/paint changes elsewhere
       (e.g. CodeMirror updates in the editor pane). Without this, every
       keystroke triggers a full layout pass over the ~thousands of SVG
       nodes in the preview. */
    contain: strict;
  }

  .preview-status-wrapper {
    display: flex;
    align-items: center;
    gap: var(--space-md);
  }

  /* A degraded render, not a failed one: the document is on screen and
     something in it is a placeholder. Amber rather than red. */
  .warning-badge {
    height: var(--control);
    padding: 0 8px;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-gray-800);
    background: light-dark(#fef3c7, #4a3a10);
    border: 1px solid light-dark(#fcd34d, #7c6218);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .warning-list {
    position: absolute;
    z-index: 5;
    top: var(--pane-toolbar-height);
    left: var(--space-sm);
    right: var(--space-sm);
    max-height: 40%;
    overflow: auto;
    margin: 0;
    padding: var(--space-sm) var(--space-md) var(--space-sm) var(--space-xl);
    font-size: 0.75rem;
    line-height: 1.7;
    color: var(--color-gray-800);
    background: var(--color-white);
    border: 1px solid light-dark(#fcd34d, #7c6218);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    overflow-wrap: anywhere;
  }

  .error-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 12px;
    animation: fadeIn 0.2s ease-out;
    background: var(--color-danger-bg);
    color: var(--color-danger);
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .preview-placeholder {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--preview-bg);
  }

  /* SVG Preview */
  .svg-preview-container {
    position: absolute;
    inset: 0;
    overflow: auto;
    padding: var(--space-lg);
    background: var(--preview-bg);
  }

  /* Kept mounted while the document view is showing, so switching back does
     not have to re-mount and re-measure every page. */
  .svg-preview-container.hidden {
    visibility: hidden;
    pointer-events: none;
  }

  .html-preview-container {
    position: absolute;
    inset: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }

  /* One box per page. It keeps the page's footprint whether or not the page
     itself is currently mounted, so scrolling and document height stay put. */
  .svg-preview-container :global(.page-slot) {
    margin: 0 auto var(--space-md);
    box-shadow: var(--paper-shadow);
    background: white;
    width: calc(100% * var(--svg-scale, 1));
    contain: strict;
  }
  .svg-preview-container :global(.page-slot > svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* Shared glyph definitions: referenced by the pages, never displayed. */
  .svg-preview-container :global(svg.typst-defs) {
    position: absolute;
    width: 0;
    height: 0;
    overflow: hidden;
  }

  .menu-container {
    position: relative;
    display: inline-block;
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    width: 150px;
    background: var(--color-white);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    z-index: 1000;
    padding: var(--space-xs) 0;
    display: flex;
    flex-direction: column;
  }

  .menu-item {
    display: flex;
    align-items: center;
    width: 100%;
    padding: var(--space-xs) var(--space-sm);
    font-size: 0.8125rem;
    color: var(--color-gray-700);
    background: transparent;
    border: none;
    text-align: left;
    cursor: pointer;
    text-decoration: none;
    transition: background-color var(--transition-fast);
  }

  .menu-item:hover {
    background-color: var(--color-gray-50);
    color: var(--color-gray-900);
  }

  .menu-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: light-dark(rgba(15, 23, 42, 0.45), rgba(0, 0, 0, 0.66));
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }
  .modal-dialog {
    background: var(--color-white);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-md);
    padding: var(--space-lg) var(--space-lg) var(--space-md);
    width: min(420px, calc(100vw - var(--space-xl)));
    box-shadow: var(--shadow-md);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .modal-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: var(--color-gray-900);
  }
  .modal-help {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--color-gray-600);
    line-height: 1.45;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-sm);
    margin-top: var(--space-xs);
  }

  .mobile-tabs {
    display: none;
  }

  @media (max-width: 768px) {
    .app {
      height: 100dvh;
    }

    .navbar {
      padding: 0 var(--space-sm);
    }

    .workspace {
      flex-direction: column;
      position: relative;
    }

    .pane {
      width: 100% !important;
      height: 100%;
      position: absolute;
      inset: 0;
      z-index: 1;
      padding-bottom: calc(50px + env(safe-area-inset-bottom));
    }

    .pane.mobile-hidden {
      display: none;
      z-index: 0;
    }

    .resizer,
    /* The bottom Editor|Preview tabs already do this job on a phone. */
    .layout-switch {
      display: none;
    }

    /* The bar is fixed to the bottom edge, which on a phone with a home
       indicator is under it — the inset moves the buttons clear while the
       background still runs to the edge. */
    .mobile-tabs {
      display: flex;
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      height: calc(50px + env(safe-area-inset-bottom));
      padding-bottom: env(safe-area-inset-bottom);
      background: var(--color-white);
      border-top: 1px solid var(--color-gray-200);
      z-index: 100;
    }

    .mobile-tab-btn {
      flex: 1;
      border: none;
      background: transparent;
      font-size: 0.875rem;
      font-weight: 500;
      color: var(--color-gray-500);
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      position: relative;
    }

    .mobile-tab-btn.active {
      color: var(--color-gray-900);
      background: var(--color-gray-50);
    }

    .mobile-tab-btn.active::after {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: var(--color-gray-900);
    }

    .hidden-mobile {
      display: none !important;
    }
  }
</style>
