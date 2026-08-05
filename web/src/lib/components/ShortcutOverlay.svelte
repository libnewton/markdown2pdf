<script lang="ts">
  import { SLASH_COMMANDS } from '$lib/editor/commands'

  let { onClose }: { onClose: () => void } = $props()

  // `Mod` renders as the key the reader actually has.
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)
  const mod = mac ? '⌘' : 'Ctrl'
  const alt = mac ? '⌥' : 'Alt'

  const groups = [
    {
      title: 'Document',
      items: [
        [`${mod}+${alt}+N`, 'New document'],
        [`${mod}+E`, 'Export menu'],
        [`${mod}+P`, 'Download PDF'],
        [`${mod}+S`, 'Not needed — every change saves itself'],
      ],
    },
    {
      title: 'View',
      items: [
        [`${mod}+1 / 2 / 3`, 'Editor / split / document'],
        [`${mod}+\\`, 'Switch Pages and Web'],
        [`${mod}+O`, 'Outline'],
        [`${mod}+Enter`, 'Refresh the preview now'],
      ],
    },
    {
      title: 'Writing',
      items: [
        [`${mod}+B`, 'Bold'],
        [`${mod}+I`, 'Italic'],
        [`${mod}+K`, 'Link'],
      ],
    },
    {
      title: 'Help',
      items: [
        ['?', 'This list'],
        ['Esc', 'Close whatever is open'],
      ],
    },
  ]
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" onclick={onClose}>
  <div
    class="modal-dialog shortcuts"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-label="Keyboard shortcuts"
    tabindex="-1"
  >
    <h3 class="modal-title">Keyboard shortcuts</h3>
    <div class="grid">
      {#each groups as group (group.title)}
        <section>
          <h4>{group.title}</h4>
          <dl>
            {#each group.items as [keys, what] (keys)}
              <dt><kbd>{keys}</kbd></dt>
              <dd>{what}</dd>
            {/each}
          </dl>
        </section>
      {/each}
      <section>
        <h4>Slash commands</h4>
        <p class="hint">Type on an empty line, then Enter.</p>
        <dl>
          {#each SLASH_COMMANDS as command (command.name)}
            <dt><kbd>/{command.name}</kbd></dt>
            <dd>{command.label}</dd>
          {/each}
        </dl>
      </section>
    </div>
    <div class="modal-actions">
      <button class="btn btn-primary btn-sm" onclick={onClose}>Close</button>
    </div>
  </div>
</div>

<style>
  .shortcuts {
    max-width: 40rem;
    width: min(40rem, calc(100vw - 2rem));
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: var(--space-lg) var(--space-xl);
    margin: var(--space-md) 0 var(--space-lg);
  }

  h4 {
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--color-gray-500);
    margin-bottom: var(--space-sm);
  }

  .hint {
    font-size: 0.75rem;
    color: var(--color-gray-500);
    margin: calc(-1 * var(--space-xs)) 0 var(--space-sm);
  }

  /* Two columns so the keys line up down the page rather than per row. */
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: baseline;
    gap: 0.35rem var(--space-md);
    margin: 0;
  }

  dd {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--color-gray-700);
  }

  kbd {
    display: inline-block;
    padding: 0.15em 0.45em;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.5;
    white-space: nowrap;
    color: var(--color-gray-800);
    background: var(--color-gray-50);
    border: 1px solid var(--color-gray-200);
    border-bottom-width: 2px;
    border-radius: var(--radius-sm);
  }
</style>
