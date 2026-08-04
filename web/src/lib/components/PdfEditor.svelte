<script lang="ts">
  import { browser } from '$app/environment'
  import { onMount } from 'svelte'
  import { buildHeadElement, buildPageElement } from '$lib/typst/svg-utils'
  import type { SvgDocument, SvgPage } from '$lib/typst/svg-split'
  import { getSharedTypstWorkerClient, TypstWorkerClient } from '$lib/workers/typstClient'
  import { getMarkdownImportFile, getImageDropFile } from '$lib/utils/image-utils'
  import { PAGEBREAK_TOKEN } from '$lib/pagebreak'
  import { cachedRemoteImages, prefetchRemoteImages } from '$lib/utils/remote-images'
  import { SUPERSEDED } from '$lib/workers/compileProtocol'
  import { useRegisterSW } from 'virtual:pwa-register/svelte'
  import type { RegisterSWOptions } from 'virtual:pwa-register/svelte'
  import { writable, type Readable } from 'svelte/store'

  import StatusHint from '$lib/components/StatusHint.svelte'
  import EditorPane from '$lib/components/EditorPane.svelte'
  import DocumentMenu from '$lib/components/DocumentMenu.svelte'
  import { PDF_TEMPLATES } from '$lib/templates/pdf-templates'
  import {
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
  }

  let {
    seoTitle,
    seoDescription,
    initialMarkdown = '',
  }: Props = $props()

  // ========================================
  // State
  // ========================================
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
  const swOptions: RegisterSWOptions = {
    onRegistered(swr) {
      console.log('SW registered: ', swr)
      if (swr) {
        setInterval(
          () => {
            console.log('Checking for SW update...')
            swr.update()
          },
          60 * 60 * 1000,
        )
      }
    },
    onRegisterError(error) {
      console.log('SW registration error', error)
    },
  }
  let needRefresh: Readable<boolean> = writable(false)
  let updateServiceWorker: (reload?: boolean) => void = () => {}
  if (browser && import.meta.env.PROD) {
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
      const pdfDocs = documentStore.recentDocuments.filter((d) => d.mode === 'pdf')
      const invalidAutoDocs = pdfDocs.filter(
        (doc) => isLegacyImplicitBlankDocument(doc) || isBrokenTemplateDocument(doc),
      )
      if (invalidAutoDocs.length > 0) {
        for (const doc of invalidAutoDocs) {
          await documentStore.deleteDocument(doc.id)
        }
      }
      const usablePdfDocs = pdfDocs.filter(
        (doc) => !isLegacyImplicitBlankDocument(doc) && !isBrokenTemplateDocument(doc),
      )
      // Try loading current doc if it belongs to this mode
      if (documentStore.currentDocId) {
        const currentDoc = documentStore.recentDocuments.find((d) => d.id === documentStore.currentDocId)
        if (currentDoc?.mode === 'pdf' && !isLegacyImplicitBlankDocument(currentDoc)) {
          const doc = await documentStore.loadDocument(documentStore.currentDocId)
          if (doc !== null) {
            applyLoadedDocument(doc)
            return
          }
        }
      }
      // Check if there are recent PDF docs
      if (usablePdfDocs.length > 0) {
        const doc = await documentStore.loadDocument(usablePdfDocs[0].id)
        if (doc !== null) {
          applyLoadedDocument(doc)
          return
        }
      }
      // Create new doc from initial markdown or template
      const defaultContent = initialMarkdown || PDF_TEMPLATES[0]?.content || ''
      markdown = defaultContent
      const doc = await documentStore.createDocument('pdf', defaultContent, undefined, 'template')
      applyLoadedDocument(doc)
    })()
  })

  // Layout state
  let leftPaneWidth = $state(50)
  let isResizing = $state(false)
  let isDragging = $state(false)



  let templates = $derived(PDF_TEMPLATES)

  // Mobile state
  let activeMobileTab = $state<'editor' | 'preview'>('editor')
  let isMenuOpen = $state(false)
  let isCorsModalOpen = $state(false)
  let corsModalDraft = $state('')
  let corsModalDialogEl = $state<HTMLDivElement | null>(null)

  function openCorsModal() {
    corsModalDraft = settingsStore.corsProxy
    isCorsModalOpen = true
    requestAnimationFrame(() => corsModalDialogEl?.focus())
    closeMenu()
  }

  function saveCorsProxy() {
    settingsStore.setCorsProxy(corsModalDraft)
    isCorsModalOpen = false
  }

  function cancelCorsProxy() {
    isCorsModalOpen = false
  }

  function toggleMenu(e?: Event) {
    if (e) {
      e.stopPropagation()
      e.preventDefault()
    }
    isMenuOpen = !isMenuOpen
  }

  function closeMenu() {
    isMenuOpen = false
  }

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
  let activePreview = $state<'pdf' | 'html'>('pdf')
  let htmlPreview = $state('')
  let htmlIframeEl = $state<HTMLIFrameElement | null>(null)
  let htmlScrollTop = 0
  let pdfCompiledMarkdown = ''
  let pdfCompiledPageNumbers: boolean | null = null
  let htmlCompiledMarkdown = ''
  let svgContainerEl = $state<HTMLDivElement | null>(null)
  let svgPageCount = $state(0)
  let svgScale = $state(1)

  // Auto-compile
  let compileSeq = 0
  let hasEverCompiled = false

  // Cached last compiled Markdown + images for PDF export (same as preview)
  let lastCompiledMarkdown = ''
  let lastCompiledImages: ImageMap = {}
  let autoPreviewTimer: number | null = null
  // The auto-compile debounce follows the last compile's duration, so a slow
  // document is not re-queued faster than it can be rendered.
  let lastCycleMs = 0

  const UI = {
    loading: 'Initializing rendering engine...',
    generating: 'Generating...',
    placeholder: 'Type Markdown here...',
  }

  function hasActivePreview(): boolean {
    return activePreview === 'pdf' ? previewDoc !== null : htmlPreview !== ''
  }
  function t<K extends keyof typeof UI>(key: K): string {
    return UI[key]
  }

  $effect(() => {
    if (!browser) return

    if (compilingHintTimer !== null) {
      window.clearTimeout(compilingHintTimer)
      compilingHintTimer = null
    }

    showPreviewCompilingHint = false

    if (status === 'compiling' && hasActivePreview()) {
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

  $effect(() => {
    if (!browser) return
    const theme = settingsStore.theme
    document.documentElement.dataset.theme = theme
    document.documentElement.style.colorScheme = theme
    document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')?.setAttribute(
      'content',
      theme === 'dark' ? '#0d1218' : '#ffffff',
    )
    htmlIframeEl?.contentWindow?.postMessage({ type: 'md2pdf-theme', theme }, '*')
  })

  // ========================================
  // Lifecycle
  // ========================================
  onMount(() => {
    loadingText = t('loading')
    void prepareCompilerRuntime().then((ready) => {
      if (!ready) return
      client = getSharedTypstWorkerClient()
      isLoading = false
    })

    // Close menus on click outside. Guarded so the global click listener
    // doesn't reactively touch state on every click in the editor — Svelte
    // 5 already short-circuits identical writes, but skipping the call
    // entirely keeps this listener off the hot path.
    const handleClickOutside = () => {
      if (isMenuOpen) closeMenu()
    }
    window.addEventListener('click', handleClickOutside)

    // Ctrl/Cmd+Enter triggers an immediate compile. Use capture phase +
    // stopImmediatePropagation so CodeMirror's editor keymap never sees it
    // and never inserts a newline.
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isCorsModalOpen) {
        cancelCorsProxy()
        return
      }
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault()
        e.stopImmediatePropagation()
        e.stopPropagation()
        compileNow()
      }
    }
    window.addEventListener('keydown', handleKey, true)

    const handlePreviewMessage = (event: MessageEvent) => {
      if (event.source !== htmlIframeEl?.contentWindow) return
      if (event.data?.type === 'md2pdf-ready') syncHtmlPreview()
      if (event.data?.type === 'md2pdf-scroll') htmlScrollTop = Number(event.data.top) || 0
      if (event.data?.type === 'md2pdf-theme-change') {
        settingsStore.setTheme(event.data.theme === 'dark' ? 'dark' : 'light')
      }
    }
    window.addEventListener('message', handlePreviewMessage)

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
      window.removeEventListener('message', handlePreviewMessage)
      if (resizeTimer) clearTimeout(resizeTimer)
    }
  })

  async function prepareCompilerRuntime(): Promise<boolean> {
    if (crossOriginIsolated || !('serviceWorker' in navigator)) {
      sessionStorage.removeItem('md2pdf-isolation-reload')
      return true
    }
    if (!import.meta.env.PROD) return true
    if (!navigator.serviceWorker.controller) {
      await Promise.race([
        navigator.serviceWorker.ready.then(() => new Promise<void>((resolve) => {
          if (navigator.serviceWorker.controller) resolve()
          else navigator.serviceWorker.addEventListener('controllerchange', () => resolve(), { once: true })
        })),
        new Promise<void>((resolve) => window.setTimeout(resolve, 8000)),
      ])
    }
    if (navigator.serviceWorker.controller && !sessionStorage.getItem('md2pdf-isolation-reload')) {
      sessionStorage.setItem('md2pdf-isolation-reload', '1')
      location.reload()
      return false
    }
    return true
  }

  function syncHtmlPreview() {
    htmlIframeEl?.contentWindow?.postMessage(
      { type: 'md2pdf-theme', theme: settingsStore.theme },
      '*',
    )
    htmlIframeEl?.contentWindow?.postMessage(
      { type: 'md2pdf-scroll-restore', top: htmlScrollTop },
      '*',
    )
  }

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

  // Auto-compile effect (debounce 450ms). Gated by the live-update setting:
  // first compile always runs so the user sees something; later ones obey the toggle.
  // Important: subscribe to `markdown` ONLY when we'll actually use it. When live
  // is paused (after the first compile), reading markdown would make every
  // keystroke re-run this effect for nothing.
  $effect(() => {
    if (!browser) return
    if (!client) return
    if (isLoading) return

    const live = settingsStore.liveUpdate
    if (hasEverCompiled && !live) return

    const md = markdown
    const target = activePreview
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const _pn = settingsStore.pageNumbers
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const _cp = settingsStore.corsProxy

    if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)

    const delay = hasEverCompiled ? Math.min(Math.max(450, lastCycleMs), 2500) : 0
    autoPreviewTimer = window.setTimeout(() => {
      if (target === activePreview) void compile(md)
    }, delay)

    return () => {
      if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)
    }
  })

  function compileNow() {
    // Compile what is on screen, including keystrokes still buffered.
    editorPane?.flushPendingEdit()
    void compile(markdown)
  }

  // Auto-fit on mobile tab switch
  $effect(() => {
    if (!browser) return
    if (activeMobileTab === 'preview' && activePreview === 'pdf') {
      setTimeout(() => {
        fitWidth()
      }, 50)
    }
  })

  // Only the pages near the viewport become DOM; the rest stay as markup
  // behind correctly-sized placeholders and mount on scroll. That keeps the
  // per-compile cost proportional to what is on screen, not to the document
  // length — a 40-page document is otherwise ~240k nodes to rebuild.
  let pageSlots: HTMLDivElement[] = []
  let pageMarkup: SvgPage[] = []
  const visibleSlots = new Set<number>()
  let headEl: SVGSVGElement | null = null
  let pageObserver: IntersectionObserver | null = null
  let observerContainer: HTMLDivElement | null = null

  function mountPage(index: number) {
    const slot = pageSlots[index]
    const page = pageMarkup[index]
    if (!slot || !page) return
    slot.replaceChildren(buildPageElement(page))
  }

  function ensureObserver(container: HTMLDivElement): IntersectionObserver {
    if (pageObserver && observerContainer === container) return pageObserver
    pageObserver?.disconnect()
    visibleSlots.clear()
    observerContainer = container
    pageObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const index = Number((entry.target as HTMLElement).dataset.page)
          if (entry.isIntersecting) {
            visibleSlots.add(index)
            if (!entry.target.firstChild) mountPage(index)
          } else {
            visibleSlots.delete(index)
            entry.target.replaceChildren()
          }
        }
      },
      // Mount a screenful ahead so scrolling finds pages already rendered.
      { root: container, rootMargin: '100% 0px' },
    )
    return pageObserver
  }

  function syncSlots(container: HTMLDivElement, pages: SvgPage[]) {
    const observer = ensureObserver(container)

    while (pageSlots.length > pages.length) {
      const slot = pageSlots.pop()!
      observer.unobserve(slot)
      visibleSlots.delete(pageSlots.length)
      slot.remove()
    }

    for (let i = 0; i < pages.length; i++) {
      let slot = pageSlots[i]
      if (!slot) {
        slot = document.createElement('div')
        slot.className = 'page-slot'
        slot.dataset.page = String(i)
        container.appendChild(slot)
        pageSlots[i] = slot
        observer.observe(slot)
      }
      // The placeholder keeps the page's footprint, so scroll position and
      // document height do not jump while pages are unmounted.
      slot.style.aspectRatio = `${pages[i].width} / ${pages[i].height}`
    }
  }

  $effect(() => {
    if (!browser) return
    const doc = previewDoc
    const container = svgContainerEl
    if (!doc || !container) return

    pageMarkup = doc.pages
    svgPageCount = doc.pages.length

    const nextHead = buildHeadElement(doc.head)
    if (headEl?.parentNode === container) {
      container.replaceChild(nextHead, headEl)
    } else {
      container.replaceChildren(nextHead)
      pageSlots = []
      visibleSlots.clear()
    }
    headEl = nextHead

    syncSlots(container, doc.pages)
    for (const index of visibleSlots) mountPage(index)
  })

  // ========================================
  // Functions
  // ========================================
  // All Markdown processing happens inside the Typst compile (the md2pdf
  // engine). The host only resolves images Typst cannot fetch itself.
  async function compile(md: string) {
    if (!client) return
    const target = activePreview
    const pageNumbers = settingsStore.pageNumbers
    // Only mark "compiled" once real content exists — otherwise a first
    // compile that races ahead of the document load would, while live
    // update is paused, leave the preview blank until a manual Update.
    if (md.trim() !== '') hasEverCompiled = true

    const seq = ++compileSeq
    // Whatever is still queued in the worker is already stale.
    client.cancelPendingPreview()
    status = 'compiling'
    errorMessage = null
    const startedAt = performance.now()

    try {
      const localImages = collectReferencedImageAssets(md)
      // Remote images are not awaited: compile with what is cached, then
      // recompile if a fetch brings in something new.
      const { images: remoteImages, missing } = cachedRemoteImages(md)
      const images: ImageMap = { ...localImages, ...remoteImages }

      lastCompiledMarkdown = md
      lastCompiledImages = images

      if (missing.length > 0) {
        void prefetchRemoteImages(missing, settingsStore.corsProxy).then((gotNew) => {
          if (gotNew && lastCompiledMarkdown === md) void compile(md)
        })
      }

      const result = target === 'pdf'
        ? await client.compilePreview(md, imagesToSend(images), pageNumbers)
        : await client.compileHtml(md, imagesToSend(images), pageNumbers)
      if (seq !== compileSeq) return
      if (target === 'pdf' && 'preview' in result) {
        previewDoc = result.preview
        pdfCompiledMarkdown = md
        pdfCompiledPageNumbers = pageNumbers
      }
      if (target === 'html' && 'html' in result) {
        htmlPreview = result.html
        htmlCompiledMarkdown = md
      }
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

  function selectPreview(target: 'pdf' | 'html') {
    if (activePreview === target) return
    editorPane?.flushPendingEdit()
    activePreview = target
    if (
      (target === 'pdf' && (
        pdfCompiledMarkdown !== markdown || pdfCompiledPageNumbers !== settingsStore.pageNumbers
      )) ||
      (target === 'html' && htmlCompiledMarkdown !== markdown)
    ) {
      void compile(markdown)
    } else if (target === 'html') {
      requestAnimationFrame(syncHtmlPreview)
    } else {
      requestAnimationFrame(fitWidth)
    }
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

  async function exportDocument() {
    editorPane?.flushPendingEdit()
    if (!client || markdown.trim() === '') return
    if (activePreview === 'html') {
      let html = htmlPreview
      if (htmlCompiledMarkdown !== markdown) {
        const localImages = collectReferencedImageAssets(markdown)
        const { images: remoteImages } = cachedRemoteImages(markdown)
        html = (await client.compileHtml(
          markdown,
          imagesToSend({ ...localImages, ...remoteImages }),
          settingsStore.pageNumbers,
        )).html
        htmlPreview = html
        htmlCompiledMarkdown = markdown
      }
      const url = URL.createObjectURL(new Blob([html], { type: 'text/html;charset=utf-8' }))
      const anchor = document.createElement('a')
      anchor.href = url
      anchor.download = 'document.html'
      anchor.click()
      setTimeout(() => URL.revokeObjectURL(url), 1000)
      return
    }
    // Open the tab synchronously so it counts as user-initiated and isn't
    // blocked by popup blockers after the async compile.
    const newTab = window.open('', '_blank')
    const images = lastCompiledMarkdown === markdown
      ? lastCompiledImages
      : { ...collectReferencedImageAssets(markdown), ...cachedRemoteImages(markdown).images }
    const { pdf } = await client.compilePdf(markdown, imagesToSend(images), settingsStore.pageNumbers)
    const blob = new Blob([pdf], { type: 'application/pdf' })
    const url = URL.createObjectURL(blob)
    if (newTab) {
      newTab.location.href = url
    } else {
      window.open(url, '_blank')
    }
    setTimeout(() => URL.revokeObjectURL(url), 60_000)
  }

  let fileInputEl = $state<HTMLInputElement | null>(null)

  function handleOpenFile() {
    fileInputEl?.click()
  }

  function onFileSelected(e: Event) {
    const target = e.target as HTMLInputElement
    const files = target.files
    if (!files || files.length === 0) return

    const file = files[0]
    const reader = new FileReader()
    reader.onload = (evt) => {
      const content = evt.target?.result
      if (typeof content === 'string') {
        markdown = content
      }
    }
    reader.readAsText(file)

    // Reset value so same file can be selected again
    target.value = ''
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

    return Object.fromEntries(
      [...referenced].map((path) => [path, imageAssets[path].bytes]),
    )
  }

  function handleHelp() {
    const defaultContent = PDF_TEMPLATES[0]?.content || ''
    if (markdown.trim() !== '' && markdown !== defaultContent) {
      if (!confirm('This will overwrite current content. Continue?')) return
    }
    markdown = defaultContent
  }

  // ========================================
  // Resizer Logic
  // ========================================
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

  // ========================================
  // Drag & Drop Logic
  // ========================================
  function hasFiles(e: DragEvent): boolean {
    return e.dataTransfer?.types?.includes('Files') ?? false
  }

  function handleDragOver(e: DragEvent) {
    if (!hasFiles(e)) return
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

    const files = e.dataTransfer?.files
    if (!files || files.length === 0) return

    const markdownFile = getMarkdownImportFile(files)
    if (markdownFile) {
      const reader = new FileReader()
      reader.onload = (event) => {
        const content = event.target?.result
        if (typeof content === 'string') {
          markdown = content
        }
      }
      reader.readAsText(markdownFile)
      return
    }

    const imageFile = getImageDropFile(files)
    if (imageFile) {
      void editorPane?.insertImageFile(imageFile)
    }
  }

  async function handlePaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items
    if (!items) return

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
  class:resizing={isResizing}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onpaste={handlePaste}
  role="application"
>
  {#if isDragging}
    <div class="drop-overlay">
      <div class="drop-overlay-content">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
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
    accept=".md,.markdown,.txt"
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
      <DocumentMenu
        mode="pdf"
        templates={PDF_TEMPLATES}
        currentContent={markdown}
        {documentAssets}
        onDocumentLoad={(doc) => { applyLoadedDocument(doc) }}
      />
    </div>
    <div class="navbar-right">
      <button
        class="btn btn-ghost btn-sm btn-icon"
        onclick={() => settingsStore.setTheme(settingsStore.theme === 'dark' ? 'light' : 'dark')}
        aria-label={settingsStore.theme === 'dark' ? 'Use light theme' : 'Use dark theme'}
        aria-pressed={settingsStore.theme === 'dark'}
        title={settingsStore.theme === 'dark' ? 'Use light theme' : 'Use dark theme'}
      >
        {settingsStore.theme === 'dark' ? '☀' : '☾'}
      </button>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="menu-container" onclick={(e) => e.stopPropagation()}>
        <button
          class="btn btn-ghost btn-sm btn-icon"
          class:active={isMenuOpen}
          onclick={toggleMenu}
          aria-label="Menu"
          style="color: var(--color-gray-900);"
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="2" fill="currentColor" stroke="none"></circle>
            <circle cx="19" cy="12" r="2" fill="currentColor" stroke="none"></circle>
            <circle cx="5" cy="12" r="2" fill="currentColor" stroke="none"></circle>
          </svg>
        </button>

        {#if isMenuOpen}
          <div class="dropdown-menu">
            <button
              class="menu-item"
              onclick={() => { handleOpenFile(); closeMenu() }}
            >
              <span class="menu-icon">📂</span>
              Open Local File
            </button>

            <button
              class="menu-item"
              onclick={() => { handleHelp(); closeMenu() }}
            >
              <span class="menu-icon">❓</span>
              Help & Guide
            </button>

            <div class="menu-divider"></div>

            <button
              class="menu-item menu-toggle"
              onclick={(e) => { e.stopPropagation(); settingsStore.setPageNumbers(!settingsStore.pageNumbers) }}
              title="Frontmatter `pageNumbers:` overrides this setting"
            >
              <span class="menu-toggle-label">Page numbers</span>
              <span class="switch" class:on={settingsStore.pageNumbers} aria-hidden="true">
                <span class="switch-thumb"></span>
              </span>
            </button>

            <button
              class="menu-item"
              onclick={openCorsModal}
              title="Optional CORS proxy for fetching images blocked by CORS"
            >
              <span class="menu-icon">🔗</span>
              CORS proxy{settingsStore.corsProxy ? ' ✓' : '…'}
            </button>

            <div class="menu-divider"></div>

            <a
              href="https://github.com/libnewton/markdown2pdf"
              target="_blank"
              rel="noopener noreferrer"
              class="menu-item"
            >
              <span class="menu-icon">🐙</span>
              GitHub
            </a>
            {#if $needRefresh}
              <div class="menu-divider"></div>
              <button
                class="menu-item"
                onclick={() => updateServiceWorker(true)}
                style="color: var(--color-green-600);"
              >
                <span class="menu-icon">⚡</span>
                Update Available
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </nav>

  <!-- Workspace -->
  <main class="workspace">
    <!-- Editor Pane -->
    <section
      class="pane editor-pane"
      class:mobile-hidden={activeMobileTab !== 'editor'}
      style="width: {leftPaneWidth}%"
    >
      <EditorPane
        bind:this={editorPane}
        bind:markdown
        placeholder={t('placeholder')}
        {errorMessage}
        pageBreakToken={PAGEBREAK_TOKEN}
        pageBreakLabel="Break"
        pageBreakTitle="Insert page break"
        onImageSaved={handleImageSaved}
      />
    </section>

    <!-- Resizer -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="resizer hidden-mobile"
      class:active={isResizing}
      onmousedown={startResize}
      onkeydown={(e) => {
        const next = e.key === 'ArrowLeft' ? leftPaneWidth - 2
          : e.key === 'ArrowRight' ? leftPaneWidth + 2
            : e.key === 'Home' ? 20
              : e.key === 'End' ? 80
                : leftPaneWidth
        if (next === leftPaneWidth) return
        e.preventDefault()
        leftPaneWidth = Math.min(Math.max(next, 20), 80)
      }}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize editor and preview"
      aria-valuemin="20"
      aria-valuemax="80"
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
      class:mobile-hidden={activeMobileTab !== 'preview'}
      style="width: {100 - leftPaneWidth}%"
    >
      <div class="preview-toolbar">
        <div class="preview-status-wrapper">
          <div class="preview-target-tabs" role="tablist" aria-label="Preview format">
            <button
              role="tab"
              aria-selected={activePreview === 'pdf'}
              class:active={activePreview === 'pdf'}
              onclick={() => selectPreview('pdf')}
            >PDF</button>
            <button
              role="tab"
              aria-selected={activePreview === 'html'}
              class:active={activePreview === 'html'}
              onclick={() => selectPreview('html')}
            >HTML</button>
          </div>
          <button
            class="live-toggle"
            class:on={settingsStore.liveUpdate}
            onclick={() => settingsStore.setLiveUpdate(!settingsStore.liveUpdate)}
            title={settingsStore.liveUpdate ? 'Pause live preview' : 'Enable live preview'}
          >
            <span class="live-dot" aria-hidden="true"></span>
            {settingsStore.liveUpdate ? 'Live' : 'Paused'}
          </button>
          {#if activePreview === 'pdf'}
            <span class="page-info">{svgPageCount || '—'} {svgPageCount === 1 ? 'page' : 'pages'}</span>
          {/if}
          {#if status === 'error'}
            <div class="error-badge">
              <span>⚠️ Failed</span>
            </div>
          {/if}
        </div>
        <div class="preview-toolbar-right">
          {#if !settingsStore.liveUpdate}
            <button
              class="btn-icon-sm"
              onclick={compileNow}
              disabled={status === 'compiling'}
              title="Compile now"
              style="padding: 4px 10px; font-size: 0.75rem;"
            >
              Update
            </button>
          {/if}
          {#if activePreview === 'pdf'}
            <div class="zoom">
              <button onclick={svgZoomOut} disabled={svgScale <= 0.25}>-</button>
              <span class="zoom-level">{Math.round(svgScale * 100)}%</span>
              <button onclick={svgZoomIn} disabled={svgScale >= 3}>+</button>
              <button onclick={fitWidth} disabled={!previewDoc}>Fit</button>
            </div>
          {/if}
          <button
            class="btn btn-primary btn-sm"
            onclick={exportDocument}
            disabled={!hasActivePreview() || status === 'compiling'}
          >
            {status === 'compiling' ? t('generating') : `Export ${activePreview.toUpperCase()}`}
          </button>
        </div>
      </div>
      <div class="preview-container" bind:this={previewContainerEl}>
        {#if showPreviewCompilingHint}
          <StatusHint label="Updating preview" />
        {/if}
        <div
          class="svg-preview-container"
          class:preview-hidden={activePreview !== 'pdf'}
          style="--svg-scale: {svgScale}"
          bind:this={svgContainerEl}
        ></div>
        {#if htmlPreview}
          <iframe
            class="html-preview"
            class:preview-hidden={activePreview !== 'html'}
            title="HTML document preview"
            sandbox="allow-scripts allow-popups"
            srcdoc={htmlPreview}
            bind:this={htmlIframeEl}
            onload={syncHtmlPreview}
          ></iframe>
        {/if}
        {#if status === 'compiling' && !hasActivePreview()}
          <div class="preview-placeholder">
            <div class="loading-spinner"></div>
          </div>
        {/if}
      </div>
    </section>
  </main>


  {#if isCorsModalOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-backdrop" onclick={cancelCorsProxy}>
      <div
        class="modal-dialog"
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="cors-modal-title"
        tabindex="-1"
        bind:this={corsModalDialogEl}
      >
        <h3 class="modal-title" id="cors-modal-title">CORS proxy</h3>
        <p class="modal-help">
          Used as a fallback when an image URL is blocked by CORS. The proxy is called with
          the image URL appended as <code>url=</code>:
        </p>
        <ul class="modal-help-list">
          <li><code>https://proxy.example.com/fetch</code> → <code>?url=&lt;image-url&gt;</code></li>
          <li><code>https://proxy.example.com/?key=ABC</code> → <code>&amp;url=&lt;image-url&gt;</code></li>
        </ul>
        <p class="modal-help">
          The proxy must return the raw image bytes. Leave empty to disable.
        </p>
        <input
          type="url"
          class="modal-input"
          placeholder="https://your-proxy.example.com/fetch"
          bind:value={corsModalDraft}
          onkeydown={(e) => { if (e.key === 'Enter') saveCorsProxy(); if (e.key === 'Escape') cancelCorsProxy() }}
        />
        <div class="modal-actions">
          <button class="btn btn-ghost btn-sm" onclick={cancelCorsProxy}>Cancel</button>
          <button class="btn btn-primary btn-sm" onclick={saveCorsProxy}>Save</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* ========================================
     App Container
     ======================================== */
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
    background: color-mix(in srgb, var(--color-white) 88%, transparent);
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
    color: var(--color-gray-500, #6b7280);
    font-size: 1rem;
    font-weight: 500;
    padding: 40px 60px;
    border: 2px dashed var(--color-gray-300, #d1d5db);
    border-radius: 16px;
    background: var(--color-gray-50, #f9fafb);
  }

  /* ========================================
     Navbar
     ======================================== */
  .navbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
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
    flex: 0 0 auto;
    gap: var(--space-xs);
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

  :global(html[data-theme='dark']) .logo-img {
    filter: brightness(0) invert(1);
  }

  /* Live update toggle */
  .live-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.5em;
    padding: calc(0.5rem - 1px) 0.875rem;
    font-size: 0.8125rem;
    font-weight: 500;
    font-family: var(--font-mono);
    line-height: 1;
    background: var(--color-gray-50);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    color: var(--color-gray-700);
    cursor: pointer;
  }
  .live-toggle:hover {
    background: var(--color-gray-100);
    border-color: var(--color-gray-300);
  }
  .live-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-gray-400);
  }
  .live-toggle.on .live-dot {
    background: #16a34a;
    box-shadow: 0 0 0 2px rgba(22, 163, 74, 0.18);
  }

  /* ========================================
     Workspace
     ======================================== */
  .workspace {
    flex: 1;
    display: flex;
    overflow: hidden;
    background-color: var(--color-gray-100);
  }

  /* ========================================
     Panes
     ======================================== */
  .pane {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    position: relative;
    background: var(--color-white);
  }

  /* Editor Pane */
  .editor-pane {
    background: var(--editor-bg);
    position: relative;
  }

  /* Resizer */
  .resizer {
    width: var(--divider-width);
    background: var(--color-gray-200);
    cursor: col-resize;
    flex-shrink: 0;
    position: relative;
    transition: background var(--transition-fast);
  }

  .resizer:hover,
  .resizer.active {
    background: var(--color-gray-400);
  }

  /* Preview Pane */
  .preview-pane {
    background: var(--preview-bg);
  }

  .preview-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-sm) var(--space-md);
    background: var(--color-white);
    border-bottom: 1px solid var(--color-gray-200);
  }

  .preview-toolbar-right {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .preview-target-tabs {
    display: inline-flex;
    padding: 2px;
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    background: var(--color-gray-50);
  }

  .preview-target-tabs button {
    min-width: 3rem;
    padding: 0.28rem 0.55rem;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-gray-500);
    font: 600 0.72rem/1 var(--font-mono);
    cursor: pointer;
  }

  .preview-target-tabs button.active {
    background: var(--color-white);
    color: var(--color-gray-900);
    box-shadow: var(--shadow-sm);
  }

  .preview-toolbar-right > .btn {
    padding: calc(0.5rem - 1px) 0.875rem;
    font-size: 0.8125rem;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .zoom button {
    padding: var(--space-xs) var(--space-sm);
    font-size: 0.75rem;
    background: var(--color-gray-100);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .zoom button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .page-info,
  .zoom-level {
    font-size: 0.75rem;
    color: var(--color-gray-500);
    font-family: var(--font-mono);
  }

  .btn-icon-sm {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    background: var(--color-gray-100);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    cursor: pointer;
    color: var(--color-gray-500);
  }

  .btn-icon-sm:hover {
    color: var(--color-gray-900);
    background: var(--color-gray-200);
  }

  .btn-icon-sm:disabled {
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

  .error-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 12px;
    animation: fadeIn 0.2s ease-out;
  }

  .error-badge {
    background: color-mix(in srgb, #ef4444 14%, var(--color-white));
    color: color-mix(in srgb, #ef4444 80%, var(--color-gray-900));
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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

  .html-preview {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    border: 0;
    background: var(--preview-bg);
  }

  .preview-hidden {
    display: none;
  }

  .app.resizing .html-preview {
    pointer-events: none;
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

  /* ========================================
     Menu
     ======================================== */
  .menu-container {
    position: relative;
    display: inline-block;
  }

  .btn-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    width: 200px;
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

  .menu-icon {
    margin-right: var(--space-sm);
    font-size: 1rem;
    line-height: 1;
  }

  .menu-divider {
    height: 1px;
    background: var(--color-gray-100);
    margin: var(--space-xs) 0;
  }

  /* Menu toggle with sliding switch */
  .menu-toggle {
    justify-content: space-between;
  }
  .menu-toggle-label {
    flex: 1;
    text-align: left;
  }
  .switch {
    display: inline-block;
    position: relative;
    width: 30px;
    height: 16px;
    background: var(--color-gray-300);
    border-radius: 999px;
    transition: background var(--transition-fast);
    flex-shrink: 0;
  }
  .switch.on {
    background: #16a34a;
  }
  .switch-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-white);
    transition: transform var(--transition-fast);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }
  .switch.on .switch-thumb {
    transform: translateX(14px);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }
  .modal-dialog {
    background: var(--color-white);
    border-radius: 8px;
    padding: 1.25rem 1.5rem 1rem;
    width: min(420px, calc(100vw - 2rem));
    box-shadow: 0 10px 32px rgba(15, 23, 42, 0.25);
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
  .modal-help code,
  .modal-help-list code {
    background: var(--color-gray-100);
    padding: 1px 4px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 0.85em;
  }
  .modal-help-list {
    margin: 0;
    padding-left: 1.1em;
    font-size: 0.8125rem;
    color: var(--color-gray-600);
    line-height: 1.6;
  }
  .modal-input {
    width: 100%;
    padding: 8px 10px;
    font-size: 0.875rem;
    font-family: var(--font-mono);
    border: 1px solid var(--color-gray-300);
    border-radius: var(--radius-sm);
    box-sizing: border-box;
    background: var(--color-gray-50);
    color: var(--color-gray-900);
  }
  .modal-input:focus {
    outline: none;
    border-color: var(--color-gray-500);
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
  }

  /* ========================================
     Mobile Layout
     ======================================== */
  .mobile-tabs {
    display: none;
  }

  @media (max-width: 769px) {
    .app {
      height: 100dvh;
    }

    .navbar {
      padding: 0 var(--space-sm);
    }

    .preview-toolbar {
      gap: var(--space-sm);
      padding-inline: var(--space-sm);
    }

    .preview-status-wrapper,
    .preview-toolbar-right {
      gap: var(--space-xs);
    }

    .page-info,
    .zoom {
      display: none;
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
      padding-bottom: 50px;
    }

    .pane.mobile-hidden {
      display: none;
      z-index: 0;
    }

    .resizer {
      display: none;
    }

    /* Mobile Tabs */
    .mobile-tabs {
      display: flex;
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      height: 50px;
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
