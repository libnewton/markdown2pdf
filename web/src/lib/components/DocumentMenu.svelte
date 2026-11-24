<script lang="ts">
  import { tick } from 'svelte'
  import { deriveNameFromContent, documentStore } from '#lib/stores/documentStore.svelte.ts'
  import type { DocumentAssets, DocumentRecord, SavedDocument } from '#lib/storage/documents.ts'
  import { trapFocus } from '#lib/utils/focus-trap.ts'

  let {
    currentContent,
    documentAssets,
    onDocumentLoad,
  }: {
    currentContent: string
    documentAssets?: DocumentAssets
    onDocumentLoad: (doc: SavedDocument) => void
  } = $props()

  let isOpen = $state(false)
  let trigger = $state<HTMLButtonElement | null>(null)
  let menu = $state<HTMLDivElement | null>(null)
  let pendingDelete = $state<DocumentRecord | null>(null)

  const currentDoc = $derived(
    documentStore.recentDocuments.find((d) => d.id === documentStore.currentDocId),
  )
  const docName = $derived(currentDoc?.name || deriveNameFromContent(currentContent))

  const menuItems = () => [...(menu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])]

  async function open() {
    isOpen = true
    await tick()
    // Start on the document that is open, which is where a reader expects to be.
    const items = menuItems()
    ;(items.find((item) => item.classList.contains('active')) ?? items[0])?.focus()
  }

  function close({ refocus = false } = {}) {
    isOpen = false
    if (refocus) trigger?.focus()
  }

  function relativeTime(ts: number): string {
    const diff = Date.now() - ts
    const minutes = Math.floor(diff / 60000)
    if (minutes < 1) return 'Just now'
    if (minutes < 60) return `${minutes}m ago`
    const hours = Math.floor(minutes / 60)
    if (hours < 24) return `${hours}h ago`
    const days = Math.floor(hours / 24)
    return `${days}d ago`
  }

  async function openDocument(doc: DocumentRecord) {
    await saveCurrentIfNeeded()
    const loadedDoc = await documentStore.loadDocument(doc.id)
    if (loadedDoc === null) return
    onDocumentLoad(loadedDoc)
    close()
  }

  /** Also reachable from Ctrl/Cmd+Alt+N and from `/new` in the editor. */
  export async function newBlank() {
    await saveCurrentIfNeeded()
    const doc = await documentStore.createDocument('', undefined, 'blank')
    onDocumentLoad(doc)
    close()
  }

  async function saveCurrentIfNeeded() {
    if (!documentStore.currentDocId) return
    await documentStore.saveNow(documentStore.currentDocId, currentContent, documentAssets)
  }

  async function deleteDocument(doc: DocumentRecord) {
    pendingDelete = null
    const wasCurrent = doc.id === documentStore.currentDocId
    await documentStore.deleteDocument(doc.id)
    // Deleting what is on screen has to leave something on screen.
    if (wasCurrent) {
      const remaining = documentStore.recentDocuments.filter((d) => d.id !== doc.id)
      if (remaining.length > 0) {
        await openDocument(remaining[0])
      } else {
        await newBlank()
      }
    }
  }

  // Arrow keys walk the items, as in any menu; Escape hands focus back to
  // the button that opened it.
  function onMenuKeydown(e: KeyboardEvent) {
    const items = menuItems()
    const at = items.indexOf(document.activeElement as HTMLElement)
    const to = {
      ArrowDown: (at + 1) % items.length,
      ArrowUp: (at - 1 + items.length) % items.length,
      Home: 0,
      End: items.length - 1,
    }[e.key]
    if (e.key === 'Escape') {
      e.preventDefault()
      close({ refocus: true })
    } else if (to !== undefined) {
      e.preventDefault()
      items[to]?.focus()
    } else if (e.key === 'Tab') {
      close()
    }
  }

  // Close on click outside
  function handleWindowClick() {
    if (isOpen) close()
  }

  const statusText = $derived(documentStore.saveStatus === 'saving' ? 'Saving...' : '')
</script>

<svelte:window onclick={handleWindowClick} />

<div class="doc-menu">
  <button
    bind:this={trigger}
    class="doc-name-btn"
    onclick={(e) => {
      e.stopPropagation()
      if (isOpen) close()
      else void open()
    }}
    onkeydown={(e) => {
      if (e.key === 'ArrowDown' && !isOpen) {
        e.preventDefault()
        void open()
      }
    }}
    title={docName}
    aria-haspopup="menu"
    aria-expanded={isOpen}
    aria-controls="doc-menu-list"
  >
    <span class="doc-name-text">{docName}</span>
    {#if statusText}
      <span class="doc-status">{statusText}</span>
    {/if}
    <svg
      class="chevron"
      class:open={isOpen}
      width="12"
      height="12"
      viewBox="0 0 12 12"
      aria-hidden="true"
    >
      <path
        d="M3 4.5L6 7.5L9 4.5"
        stroke="currentColor"
        stroke-width="1.5"
        fill="none"
        stroke-linecap="round"
      />
    </svg>
  </button>

  {#if isOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      bind:this={menu}
      id="doc-menu-list"
      class="doc-popover"
      role="menu"
      aria-label="Documents"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={onMenuKeydown}
    >
      {#if documentStore.recentDocuments.length > 0}
        <div class="doc-section-label" role="presentation">Local Documents</div>
        <div class="doc-list" role="group" aria-label="Local Documents">
          {#each documentStore.recentDocuments as doc (doc.id)}
            <div class="doc-row" class:active={doc.id === documentStore.currentDocId}>
              <button
                class="doc-item"
                class:active={doc.id === documentStore.currentDocId}
                role="menuitem"
                tabindex="-1"
                aria-current={doc.id === documentStore.currentDocId ? 'true' : undefined}
                onclick={() => openDocument(doc)}
              >
                <span class="doc-item-name">{doc.name}</span>
                <span class="doc-item-time">{relativeTime(doc.updatedAt)}</span>
              </button>
              <button
                class="doc-item-delete"
                role="menuitem"
                tabindex="-1"
                title="Delete"
                aria-label={`Delete ${doc.name || 'untitled document'}`}
                onclick={() => (pendingDelete = doc)}
              >
                &times;
              </button>
            </div>
          {/each}
        </div>
        <div class="doc-divider" role="separator"></div>
      {/if}

      <button class="doc-item doc-new" role="menuitem" tabindex="-1" onclick={newBlank}>
        + New blank document
      </button>
    </div>
  {/if}
</div>

{#if pendingDelete}
  {@const doc = pendingDelete}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="modal-backdrop"
    onclick={(e) => {
      e.stopPropagation()
      pendingDelete = null
    }}
  >
    <div
      class="modal-dialog"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        if (e.key !== 'Escape') return
        e.preventDefault()
        pendingDelete = null
      }}
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="delete-title"
      aria-describedby="delete-help"
      tabindex="-1"
      {@attach trapFocus}
    >
      <h3 class="modal-title" id="delete-title">Delete this document?</h3>
      <p class="modal-help" id="delete-help">
        <strong>{doc.name || 'Untitled document'}</strong> is removed from this browser for good.
      </p>
      <div class="modal-actions">
        <button class="btn btn-secondary btn-sm" onclick={() => (pendingDelete = null)}>
          Cancel
        </button>
        <button class="btn btn-primary btn-sm" onclick={() => deleteDocument(doc)}>Delete</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .doc-menu {
    position: relative;
    display: flex;
    align-items: center;
    max-width: 280px;
  }

  .doc-name-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: var(--control-lg);
    padding: 0 8px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: 0.8125rem;
    color: var(--color-gray-700);
    transition: all var(--transition-fast);
    max-width: 280px;
  }

  .doc-name-btn:hover {
    background: var(--color-gray-50);
    border-color: var(--color-gray-200);
  }

  .doc-name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
    font-weight: 500;
  }

  .doc-status {
    font-size: 0.8125rem;
    color: var(--color-gray-400);
    white-space: nowrap;
  }

  .chevron {
    flex-shrink: 0;
    color: var(--color-gray-400);
    transition: transform var(--transition-fast);
  }

  .chevron.open {
    transform: rotate(180deg);
  }

  .doc-popover {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 300px;
    max-height: 400px;
    overflow-y: auto;
    background: var(--color-white);
    border: 1px solid var(--color-gray-200);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    z-index: 1000;
    padding: var(--space-xs) 0;
  }

  .doc-section-label {
    padding: var(--space-xs) var(--space-sm);
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--color-gray-400);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .doc-list {
    max-height: 200px;
    overflow-y: auto;
  }

  .doc-row {
    display: flex;
    align-items: center;
    padding-right: var(--space-sm);
    transition: background-color var(--transition-fast);
  }

  .doc-row:hover {
    background: var(--color-gray-50);
  }

  .doc-row.active {
    background: var(--color-gray-100);
  }

  .doc-row .doc-item,
  .doc-row .doc-item:hover {
    flex: 1;
    min-width: 0;
    padding-right: 0;
    background: transparent;
  }

  .doc-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px var(--space-sm);
    font-size: 0.8125rem;
    color: var(--color-gray-700);
    background: transparent;
    border: none;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--transition-fast);
  }

  .doc-item:hover {
    background: var(--color-gray-50);
  }

  .doc-item.active {
    background: var(--color-gray-100);
    font-weight: 500;
  }

  .doc-item-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .doc-item-time {
    font-size: 0.6875rem;
    color: var(--color-gray-400);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* Collapsed rather than `display: none` until the row is hovered or
     focused, so the keyboard and screen readers can still reach it. */
  .doc-item-delete {
    width: 0;
    padding: 0;
    overflow: hidden;
    opacity: 0;
    font-size: 0.875rem;
    color: var(--color-gray-400);
    background: transparent;
    border: none;
    cursor: pointer;
    line-height: 1;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .doc-row:hover .doc-item-delete,
  .doc-row:focus-within .doc-item-delete {
    width: auto;
    margin-left: 6px;
    padding: 0 4px;
    opacity: 1;
  }

  .doc-item-delete:hover {
    color: var(--color-gray-700);
    background: var(--color-gray-200);
  }

  .doc-divider {
    height: 1px;
    background: var(--color-gray-100);
    margin: var(--space-xs) 0;
  }

  .doc-new {
    color: var(--color-gray-500);
  }

  @media (max-width: 768px) {
    .doc-menu {
      max-width: 160px;
    }

    .doc-name-text {
      max-width: 120px;
    }

    .doc-popover {
      width: 260px;
    }
  }
</style>
