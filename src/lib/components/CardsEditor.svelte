<script lang="ts">
  import { browser } from '$app/environment'
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { getTypstRenderer } from '$lib/typst/renderer'
  import { extractFirstPageSvg } from '$lib/typst/svg-utils'
  import { markdownToTypst, markdownToTypstPages } from '$lib/pipeline/markdownToTypst'
  import type { TypstStyleId } from '$lib/pipeline/markdownToTypst'
  import {
    DEFAULT_CARD_EXPORT_PRESET,
    getCardExportPreset,
    normalizeCardExportPresetId,
    type CardExportPresetId,
  } from '$lib/card-export-presets'
  import { getSharedTypstWorkerClient, TypstWorkerClient } from '$lib/workers/typstClient'
    import { renderMermaidToSvg } from '$lib/mermaid/render'
  import { getMarkdownImportFile, getImageDropFile } from '$lib/utils/image-utils'
  import EditorPane from '$lib/components/EditorPane.svelte'
  import CardGallery from '$lib/components/CardGallery.svelte'
  import DocumentMenu from '$lib/components/DocumentMenu.svelte'
  import { CARD_TEMPLATES } from '$lib/templates/card-templates'
  import { SOCIAL_IMAGE } from '$lib/seo'
  import {
    documentStore,
    isBrokenTemplateDocument,
    isLegacyImplicitBlankDocument,
  } from '$lib/stores/documentStore.svelte'
  import type { SavedDocument } from '$lib/storage/documents'
  import { PAGEBREAK_TOKEN } from '$lib/pagebreak'

  // Props
  interface Props {
    seoTitle: string
    seoDescription: string
  }

  let { seoTitle, seoDescription }: Props = $props()

  // ========================================
  // State
  // ========================================
  let markdown = $state('')
  let hasInitializedMarkdown = false
  let editorPane = $state<EditorPane | null>(null)

  let leftPaneWidth = $state(50)
  let isResizing = $state(false)
  let isDragging = $state(false)

  type LocalImageAsset = {
    bytes: Uint8Array
    objectUrl: string
  }

  let imageAssets = $state<Record<string, LocalImageAsset>>({})


  let style = $state<'redbook-knowledge' | 'redbook-dark' | 'redbook-minimalist' | 'redbook-modern' | 'redbook-forest' | 'redbook-blueprint' | 'redbook-clean'>(
    (browser &&
      (localStorage.getItem('md2pdf-redbook-style') as
        | 'redbook-knowledge'
        | 'redbook-dark'
        | 'redbook-minimalist'
        | 'redbook-modern'
        | 'redbook-forest'
        | 'redbook-blueprint'
        | 'redbook-clean')) ||
      'redbook-clean',
  )

  let cardTheme = $state(
    (browser && localStorage.getItem('md2pdf-card-theme')) || 'indigo',
  )

  let font = $state<'sans' | 'serif'>(
    (browser && (localStorage.getItem('md2pdf-card-font') as 'sans' | 'serif')) ||
      'sans',
  )

  let cardSize = $state<'compact' | 'regular' | 'large'>(
    (browser && (localStorage.getItem('md2pdf-card-size') as 'compact' | 'regular' | 'large')) ||
      'compact',
  )

  let cardDensity = $state<'tight' | 'comfortable' | 'relaxed'>(
    (browser &&
      (localStorage.getItem('md2pdf-card-density') as 'tight' | 'comfortable' | 'relaxed')) ||
      'comfortable',
  )

  let cardExportPreset = $state<CardExportPresetId>(DEFAULT_CARD_EXPORT_PRESET)

  // Toolbar popover state
  let openPopover = $state<'style' | 'layout' | null>(null)

  function togglePopover(name: 'style' | 'layout') {
    openPopover = openPopover === name ? null : name
  }

  function closePopovers() {
    openPopover = null
  }

  // Compilation state
  let status: 'idle' | 'compiling' | 'done' | 'error' = $state('idle')
  let errorMessage: string | null = $state(null)
  let pageSvgs = $state<string[] | null>(null)

  // Cache for incremental per-page compilation
  let cachedTypstSources: string[] = []
  let cachedSvgs: string[] = []
  let cachedImagesFingerprint = ''

  // Loading state
  let isLoading = $state(true)

  // Worker client
  let client = $state<TypstWorkerClient | null>(null)

  // Auto-compile
  let compileSeq = 0
  let hasEverCompiled = false
  let autoPreviewTimer: number | null = null

  // Mobile state
  let activeMobileTab = $state<'editor' | 'preview'>('editor')
  let isMenuOpen = $state(false)

  function applyLoadedDocument(doc: SavedDocument) {
    markdown = doc.content
    documentStore.finishDocumentTransition()
  }

  $effect(() => {
    if (hasInitializedMarkdown) return
    if (!browser) return
    hasInitializedMarkdown = true
    ;(async () => {
      const storedPreset = localStorage.getItem('md2pdf-card-export-preset')
      const normalizedPreset = normalizeCardExportPresetId(storedPreset)
      cardExportPreset = normalizedPreset ?? 'instagram-feed'
      if (cardExportPreset !== storedPreset && cardExportPreset) {
        localStorage.setItem('md2pdf-card-export-preset', cardExportPreset)
      }

      await documentStore.init()
      // Migrate old localStorage data
      const oldSaved = localStorage.getItem('md2pdf-redbook-markdown')
      if (oldSaved && oldSaved.trim()) {
        localStorage.removeItem('md2pdf-redbook-markdown')
        const doc = await documentStore.createDocument('redbook', oldSaved, undefined, 'import')
        applyLoadedDocument(doc)
        return
      }
      const redbookDocs = documentStore.recentDocuments.filter((d) => d.mode === 'redbook')
      const invalidAutoDocs = redbookDocs.filter(
        (doc) => isLegacyImplicitBlankDocument(doc) || isBrokenTemplateDocument(doc),
      )
      if (invalidAutoDocs.length > 0) {
        for (const doc of invalidAutoDocs) {
          await documentStore.deleteDocument(doc.id)
        }
      }
      const usableRedbookDocs = redbookDocs.filter(
        (doc) => !isLegacyImplicitBlankDocument(doc) && !isBrokenTemplateDocument(doc),
      )
      // Try loading current doc if it belongs to this mode
      if (documentStore.currentDocId) {
        const currentDoc = documentStore.recentDocuments.find((d) => d.id === documentStore.currentDocId)
        if (currentDoc?.mode === 'redbook' && !isLegacyImplicitBlankDocument(currentDoc)) {
          const doc = await documentStore.loadDocument(documentStore.currentDocId)
          if (doc !== null) {
            applyLoadedDocument(doc)
            return
          }
        }
      }
      // Check recent redbook docs
      if (usableRedbookDocs.length > 0) {
        const doc = await documentStore.loadDocument(usableRedbookDocs[0].id)
        if (doc !== null) {
          applyLoadedDocument(doc)
          return
        }
      }
      // Create from template
      const defaultContent = CARD_TEMPLATES[0]?.content ?? ""
      markdown = defaultContent
      const doc = await documentStore.createDocument('redbook', defaultContent, undefined, 'template')
      applyLoadedDocument(doc)
    })()
  })

  // ========================================
  // UI Text
  // ========================================
  const UI = {
    exportCards: 'Download Images',
    loading: 'Initializing rendering engine...',
    generating: 'Generating...',
    placeholder: 'Type Markdown here...',
  }

  function t<K extends keyof typeof UI>(key: K): string {
    return UI[key]
  }

  // ========================================
  // Derived
  // ========================================
  let filename = $derived.by(() => {
    const h1Match = markdown.match(/^#\s+(.+)$/m)
    let base = h1Match ? h1Match[1].trim() : ''
    base = base.replace(/[\\/:*?"<>|\x00-\x1F]/g, ' ')
    base = base.replace(/\s+/g, ' ').trim()
    if (!base) base = ''
    const MAX_LEN = 50
    if (base.length > MAX_LEN) {
      base = base.substring(0, MAX_LEN).trim()
    }
    return base
  })

  let activeCardExportPreset = $derived.by(() => getCardExportPreset(cardExportPreset))

  // ========================================
  // Lifecycle
  // ========================================
  onMount(() => {
    client = getSharedTypstWorkerClient()

    let aborted = false

    void (async () => {
      isLoading = false

      // Trigger first compile
      void compile(markdown, style, font, cardSize, cardDensity, cardTheme, cardExportPreset)
    })().catch((error) => {
      console.error(error)
      isLoading = false
    })

    // Close menu on click outside
    const handleClickOutside = () => {
      closeMenu()
    }
    window.addEventListener('click', handleClickOutside)

    return () => {
      aborted = true
      window.removeEventListener('click', handleClickOutside)
      for (const asset of Object.values(imageAssets)) {
        URL.revokeObjectURL(asset.objectUrl)
      }
    }
  })

  // ========================================
  // Persist preferences
  // ========================================
  $effect(() => {
    if (!browser) return
    localStorage.setItem('md2pdf-redbook-style', style)
    localStorage.setItem('md2pdf-card-theme', cardTheme)
    localStorage.setItem('md2pdf-card-font', font)
    localStorage.setItem('md2pdf-card-size', cardSize)
    localStorage.setItem('md2pdf-card-density', cardDensity)
    localStorage.setItem('md2pdf-card-export-preset', cardExportPreset)
  })

  // Auto-save document to IndexedDB
  $effect(() => {
    if (!browser || !hasInitializedMarkdown || !documentStore.currentDocId) return
    if (documentStore.isTransitioningDocument) return
    documentStore.autoSave(documentStore.currentDocId, markdown)
  })

  // ========================================
  // Auto-compile effect (debounce 450ms)
  // ========================================
  $effect(() => {
    if (!browser) return
    if (!client) return
    if (isLoading) return

    const md = markdown
    const _style = style
    const _font = font
    const _cardSize = cardSize
    const _cardDensity = cardDensity
    const _cardTheme = cardTheme
    const _cardExportPreset = cardExportPreset

    if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)

    const delay = hasEverCompiled ? 450 : 0
    autoPreviewTimer = window.setTimeout(() => {
      void compile(md, _style, _font, _cardSize, _cardDensity, _cardTheme, _cardExportPreset)
    }, delay)

    return () => {
      if (autoPreviewTimer) window.clearTimeout(autoPreviewTimer)
    }
  })

  // ========================================
  // Compile function
  // ========================================
  async function compile(
    md: string,
    nextStyle: TypstStyleId,
    compileFont: 'sans' | 'serif' = 'sans',
    compileSize: 'compact' | 'regular' | 'large' = 'compact',
    compileDensity: 'tight' | 'comfortable' | 'relaxed' = 'comfortable',
    compileTheme: string = 'indigo',
    compileExportPreset: CardExportPresetId = DEFAULT_CARD_EXPORT_PRESET,
  ) {
    if (!client) return
    hasEverCompiled = true

    const seq = ++compileSeq
    status = 'compiling'
    errorMessage = null

    try {
      // Pre-process Mermaid blocks
      let processedMd = md
      const images: Record<string, Uint8Array> = {}

      const mermaidRegex = /```mermaid\n([\s\S]*?)\n```/g
      const matches = [...md.matchAll(mermaidRegex)]

      if (matches.length > 0) {
        let lastIndex = 0
        let newContent = ''

        for (const [index, match] of matches.entries()) {
          const [fullMatch, code] = match
          const id = `mermaid-${index}`
          const mermaidFilename = `${id}.svg`

          try {
            const svg = await renderMermaidToSvg(code, id)
            images[mermaidFilename] = svg

            newContent += md.slice(lastIndex, match.index)
            newContent += `![Mermaid Diagram](${mermaidFilename})`
            lastIndex = (match.index || 0) + fullMatch.length
          } catch (e) {
            console.error('Mermaid render failed', e)
            newContent += md.slice(
              lastIndex,
              (match.index || 0) + fullMatch.length,
            )
            lastIndex = (match.index || 0) + fullMatch.length
          }
        }
        newContent += md.slice(lastIndex)
        processedMd = newContent
      }

      Object.assign(images, collectReferencedImageAssets(processedMd))

      const typstPages = markdownToTypstPages(processedMd, {
        style: nextStyle,
        lang: 'en',
        font: compileFont,
        size: compileSize,
        density: compileDensity,
        theme: compileTheme,
        exportPreset: compileExportPreset,
      })

      // Incremental: only recompile pages whose Typst source or images changed
      const renderer = await getTypstRenderer()
      const imagesFingerprint = Object.keys(images).sort().join(',') + ':' +
        Object.values(images).reduce((sum, v) => sum + v.length, 0)
      const imagesChanged = imagesFingerprint !== cachedImagesFingerprint
      const svgs = imagesChanged ? [] : [...cachedSvgs]

      for (let i = 0; i < typstPages.length; i++) {
        if (seq !== compileSeq) return
        if (!imagesChanged && typstPages[i] === cachedTypstSources[i] && svgs[i]) continue

        // @ts-ignore
        const { vector } = await client.compileVector(typstPages[i], images)
        await renderer.runWithSession(
          { format: 'vector' as const, artifactContent: vector },
          async (session) => {
            svgs[i] = extractFirstPageSvg(await session.renderSvg({}))
          },
        )
      }
      if (seq !== compileSeq) return

      // Trim if pages were removed
      svgs.length = typstPages.length
      cachedTypstSources = typstPages
      cachedSvgs = svgs
      cachedImagesFingerprint = imagesFingerprint
      pageSvgs = [...svgs]
      status = 'done'
    } catch (error) {
      if (seq !== compileSeq) return
      status = 'error'
      errorMessage = error instanceof Error ? error.message : String(error)
    }
  }

  // ========================================
  // Download cards using the active export preset resolution
  // ========================================
  async function downloadCards() {
    if (!pageSvgs?.length) return
    const exportPreset = getCardExportPreset(cardExportPreset)

    for (let i = 0; i < pageSvgs.length; i++) {
      const safeSvg = pageSvgs[i].replace(/&(?!(?:amp|lt|gt|quot|apos|#\d+|#x[0-9a-fA-F]+);)/g, '&amp;')
      const dataUrl = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(safeSvg)
      const img = await new Promise<HTMLImageElement>((resolve, reject) => {
        const el = new Image()
        el.onload = () => resolve(el)
        el.onerror = reject
        el.src = dataUrl
      })

      const w = exportPreset.fullResWidth
      const h = Math.round(w * img.naturalHeight / img.naturalWidth)
      const canvas = document.createElement('canvas')
      canvas.width = w
      canvas.height = h
      const ctx = canvas.getContext('2d')!
      ctx.drawImage(img, 0, 0, w, h)

      const blob = await new Promise<Blob>((resolve) =>
        canvas.toBlob((b) => resolve(b!), 'image/png'),
      )
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${filename}-${String(i + 1).padStart(2, '0')}.png`
      a.click()
      URL.revokeObjectURL(url)
    }
  }

  function handleImageSaved(path: string, bytes: Uint8Array, objectUrl: string, _mimeType: string) {
    imageAssets[path] = { bytes, objectUrl }
  }

  function collectReferencedImageAssets(md: string): Record<string, Uint8Array> {
    const referenced = new Set<string>()
    const markdownImageRegex = /!\[[^\]]*]\(([^)\s]+)(?:\s+"[^"]*")?\)/g

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

  // ========================================
  // Navigation helpers
  // ========================================

  function toggleMenu(e?: Event) {
    if (e) {
      e.stopPropagation()
      e.preventDefault()
    }
    isMenuOpen = !isMenuOpen
  }

  function closeMenu() {
    isMenuOpen = false
    closePopovers()
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

  function onResize(e: MouseEvent) {
    if (!isResizing) return
    const containerWidth = window.innerWidth
    const newWidth = (e.clientX / containerWidth) * 100
    leftPaneWidth = Math.min(Math.max(newWidth, 20), 80)
  }

  function stopResize() {
    isResizing = false
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
</script>

<svelte:head>
  <title>{seoTitle}</title>
  <meta name="description" content={seoDescription} />
  <meta property="og:title" content={seoTitle} />
  <meta property="og:description" content={seoDescription} />
  <meta property="og:type" content="website" />
  <meta property="og:image" content={SOCIAL_IMAGE} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="240" />
  <meta property="og:image:height" content="240" />
  <meta property="og:image:alt" content="md2pdf logo" />
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
  <div class="loading-text">{t('loading')}</div>
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
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="17 8 12 3 7 8"></polyline>
          <line x1="12" y1="3" x2="12" y2="15"></line>
        </svg>
        <span>Drop .md or image files here</span>
      </div>
    </div>
  {/if}
  <!-- Navbar -->
  <nav class="navbar">
    <div class="navbar-left">
      <a href="/" class="logo-link">
        <img src="/logo.png" alt="md2pdf" class="logo-img" />
      </a>
      <div class="mode-toggle hidden-mobile">
        <a href="/" class="mode-toggle-item">PDF</a>
        <span class="mode-toggle-item active">Cards</span>
        <a href="/slides/" class="mode-toggle-item">Slides</a>
      </div>
      <DocumentMenu
                mode="redbook"
        templates={CARD_TEMPLATES}
        currentContent={markdown}
        onDocumentLoad={(doc) => { applyLoadedDocument(doc) }}
      />
    </div>
    <div class="navbar-right">
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
            <a
              href="https://github.com/libnewton/markdown2pdf"
              target="_blank"
              rel="noopener noreferrer"
              class="menu-item"
            >
              <span class="menu-icon">🐙</span>
              GitHub
            </a>
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
        cardMode
        {errorMessage}
        pageBreakToken={PAGEBREAK_TOKEN}
        pageBreakLabel="Break"
        pageBreakTitle="Insert page break (new card)"
        {imageAssets}
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
      role="separator"
      aria-orientation="vertical"
      tabindex="0"
    ></div>

    <!-- Mobile Tab Switcher -->
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

    <!-- Preview Pane (Card Gallery) -->
    <section
      class="pane preview-pane"
      class:mobile-hidden={activeMobileTab !== 'preview'}
      style="width: {100 - leftPaneWidth}%"
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="preview-toolbar" onclick={(e: MouseEvent) => e.stopPropagation()}>
        <div class="preview-toolbar-row">
          <select class="style-select" bind:value={cardExportPreset}>
            <option value="redbook-portrait">Redbook 3:4</option>
            <option value="instagram-feed">Instagram 4:5</option>
            <option value="x-square">X Square 1:1</option>
            <option value="story">Story 9:16</option>
          </select>

          <div class="popover-anchor">
            <button
              class="toolbar-popover-btn"
              class:active={openPopover === 'style'}
              onclick={() => togglePopover('style')}
            >
              Style
              <span class="popover-chevron">{openPopover === 'style' ? '▴' : '▾'}</span>
            </button>
            {#if openPopover === 'style'}
              <div class="popover-dropdown">
                <select class="style-select" bind:value={style}>
                  <option value="redbook-clean">Clean</option>
                  <option value="redbook-modern">Modern</option>
                  <option value="redbook-minimalist">Minimal</option>
                  <option value="redbook-knowledge">Warm</option>
                  <option value="redbook-forest">Forest</option>
                  <option value="redbook-blueprint">Blueprint</option>
                  <option value="redbook-dark">Dark</option>
                </select>
                {#if style === 'redbook-modern'}
                  <select class="font-select" bind:value={cardTheme}>
                    <option value="indigo">Indigo</option>
                    <option value="amber">Amber</option>
                    <option value="teal">Teal</option>
                    <option value="violet">Violet</option>
                    <option value="rose">Rose</option>
                    <option value="pink">Pink</option>
                  </select>
                {/if}
              </div>
            {/if}
          </div>

          <div class="popover-anchor">
            <button
              class="toolbar-popover-btn"
              class:active={openPopover === 'layout'}
              onclick={() => togglePopover('layout')}
            >
              Layout
              <span class="popover-chevron">{openPopover === 'layout' ? '▴' : '▾'}</span>
            </button>
            {#if openPopover === 'layout'}
              <div class="popover-dropdown">
                <select class="font-select" bind:value={font}>
                  <option value="sans">Sans</option>
                  <option value="serif">Serif</option>
                </select>
                <select class="font-select" bind:value={cardSize}>
                  <option value="compact">Compact</option>
                  <option value="regular">Regular</option>
                  <option value="large">Large</option>
                </select>
                <select class="font-select" bind:value={cardDensity}>
                  <option value="tight">Tight</option>
                  <option value="comfortable">Comfortable</option>
                  <option value="relaxed">Relaxed</option>
                </select>
              </div>
            {/if}
          </div>

          <div class="preview-toolbar-spacer"></div>

          {#if status === 'compiling'}
            <div class="compiling-badge">
              <div class="spinner-xs"></div>
              <span>Generating...</span>
            </div>
          {:else if status === 'error'}
            <div class="error-badge">
              <span>Failed</span>
            </div>
          {/if}
          <button
            class="btn btn-primary btn-sm"
            onclick={downloadCards}
            disabled={!pageSvgs || status === 'compiling'}
          >
            {status === 'compiling' ? t('generating') : t('exportCards')}
          </button>
        </div>
      </div>
      <CardGallery
        {pageSvgs}
        {status}
        {filename}
                columns={0}
        aspectRatio={activeCardExportPreset.aspectRatio}
        fullResWidth={activeCardExportPreset.fullResWidth}
        thumbWidth={activeCardExportPreset.thumbWidth}
      />
    </section>
  </main>
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
    background: rgba(255, 255, 255, 0.85);
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
     Loading Overlay
     ======================================== */
  .loading-overlay {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    background: var(--color-white);
    z-index: 9999;
    transition: opacity 0.3s ease;
  }

  .loading-overlay.hidden {
    opacity: 0;
    pointer-events: none;
  }

  .loading-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid var(--color-gray-200);
    border-top-color: var(--color-gray-600);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .loading-progress {
    width: 200px;
    height: 3px;
    background: var(--color-gray-200);
    border-radius: 2px;
    overflow: hidden;
  }

  .loading-progress-bar {
    width: 40%;
    height: 100%;
    background: var(--color-gray-500);
    border-radius: 2px;
    animation: progress-indeterminate 1.2s ease-in-out infinite;
  }

  @keyframes progress-indeterminate {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(350%);
    }
  }

  .loading-text {
    font-size: 0.875rem;
    color: var(--color-gray-500);
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

  .navbar-right > .btn {
    padding-left: var(--space-md);
    padding-right: var(--space-md);
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

  .mode-toggle {
    display: flex;
    align-items: center;
    background: var(--color-gray-100);
    border-radius: var(--radius-sm);
    padding: 2px;
    gap: 0;
  }

  .mode-toggle-item {
    font-size: 0.75rem;
    font-weight: 500;
    padding: 3px 10px;
    border-radius: calc(var(--radius-sm) - 1px);
    color: var(--color-gray-500);
    text-decoration: none;
    transition: all var(--transition-fast);
    cursor: pointer;
    white-space: nowrap;
  }

  .mode-toggle-item:hover:not(.active) {
    color: var(--color-gray-700);
  }

  .mode-toggle-item.active {
    background: var(--color-white);
    color: var(--color-gray-900);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    cursor: default;
  }

  .style-select,
  .font-select {
    appearance: none;
    -webkit-appearance: none;
    padding: calc(0.5rem - 1px) 2rem calc(0.5rem - 1px) 0.875rem;
    font-size: 0.8125rem;
    font-weight: 500;
    font-family: var(--font-mono);
    line-height: 1;
    background-color: var(--color-gray-50);
    background-image: url("data:image/svg+xml,%3Csvg width='12' height='12' viewBox='0 0 24 24' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M6 9L12 15L18 9' stroke='%23737373' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.75rem center;
    background-size: 1rem;
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    cursor: pointer;
    box-sizing: border-box;
  }

  .style-select:hover,
  .font-select:hover {
    background-color: var(--color-gray-100);
    border-color: var(--color-gray-300);
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
    background: #fff;
  }

  .editor-pane {
    background: var(--editor-bg);
    position: relative;
  }

  .preview-pane {
    background: var(--preview-bg);
  }

  .preview-toolbar {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding: 4px 8px;
    border-bottom: 1px solid var(--color-gray-200, #e5e7eb);
    background: var(--color-white, #fff);
    flex-shrink: 0;
  }

  .preview-toolbar-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .preview-toolbar-spacer {
    flex: 1;
  }

  .preview-toolbar-row > .btn {
    padding: calc(0.5rem - 1px) 0.875rem;
    font-size: 0.8125rem;
    flex: 0 0 auto;
    white-space: nowrap;
  }

  /* Popover trigger button */
  .toolbar-popover-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: calc(0.5rem - 1px) 0.875rem;
    font-size: 0.8125rem;
    font-weight: 500;
    font-family: var(--font-mono);
    line-height: 1;
    background: var(--color-gray-50);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    cursor: pointer;
    white-space: nowrap;
    color: var(--color-gray-700, #374151);
    transition: all var(--transition-fast);
  }

  .toolbar-popover-btn:hover {
    background: var(--color-gray-100);
    border-color: var(--color-gray-300);
  }

  .toolbar-popover-btn.active {
    background: var(--color-gray-100);
    border-color: var(--color-gray-400);
  }

  .popover-chevron {
    font-size: 0.625rem;
    color: var(--color-gray-400);
    line-height: 1;
  }

  /* Popover anchor + dropdown */
  .popover-anchor {
    position: relative;
  }

  .popover-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--color-white, #fff);
    border: 1px solid var(--color-gray-200, #e5e7eb);
    border-radius: var(--radius-sm);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
    white-space: nowrap;
  }

  .compiling-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
    color: var(--color-gray-500, #6b7280);
  }

  .error-badge {
    font-size: 0.75rem;
    color: #ef4444;
  }

  .spinner-xs {
    width: 12px;
    height: 12px;
    border: 1.5px solid var(--color-gray-200, #e5e7eb);
    border-top-color: var(--color-gray-500, #6b7280);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* ========================================
     Resizer
     ======================================== */
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

  .menu-item.small {
    font-size: 0.75rem;
    padding: 4px var(--space-sm);
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

  /* ========================================
     Mobile Layout
     ======================================== */
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
      padding-bottom: 50px;
    }

    .pane.mobile-hidden {
      display: none;
      z-index: 0;
    }

    .resizer {
      display: none;
    }

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

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
