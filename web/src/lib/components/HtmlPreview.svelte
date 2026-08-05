<script lang="ts">
  // Pageless HTML view of the document.
  //
  // The markup and its stylesheet come from the engine — byte for byte the
  // same thing the download and the CLI produce. It is mounted in a shadow
  // root so the document's CSS and the app's CSS can never reach each other.

  let {
    html = '',
    theme,
    onnavigate,
    ontasktoggle,
    onscrolled,
  }: {
    html?: string
    theme: 'light' | 'dark'
    /** A heading the reader jumped to, so the URL can follow along. */
    onnavigate?: (id: string) => void
    /** A checkbox the reader ticked, by the source line that wrote it. */
    ontasktoggle?: (line: number, checked: boolean) => void
    /** The pane was scrolled, so the editor can follow. */
    onscrolled?: () => void
  } = $props()

  let host = $state<HTMLDivElement | null>(null)
  let root: ShadowRoot | null = null

  $effect(() => {
    if (!host) return
    if (!root) {
      root = host.attachShadow({ mode: 'open' })
      // Bound to the root rather than its children, so both outlive every
      // re-render below.
      root.addEventListener('click', onClick)
      // `change`, not `click`: it fires for keyboard activation too,
      // reports the state after the toggle, and — being composed: false —
      // stops at the root instead of leaking into the page.
      root.addEventListener('change', onChange)
    }
    mount(root, html)
  })

  // The fragment opens with a <style> holding the whole document stylesheet
  // and, when there is math, a base64 font. That block is the same on every
  // render, so re-parsing it per keystroke is pure cost — the body after it
  // is the only part that actually changes.
  let styleEl: HTMLStyleElement | null = null
  let mountedStyle = ''

  function mount(target: ShadowRoot, fragment: string) {
    const end = fragment.indexOf('</style>')
    if (!fragment.startsWith('<style>') || end === -1) {
      target.innerHTML = fragment
      styleEl = null
      return
    }
    const css = fragment.slice('<style>'.length, end)
    const body = fragment.slice(end + '</style>'.length)

    if (!styleEl || !styleEl.isConnected) {
      target.innerHTML = ''
      styleEl = document.createElement('style')
      target.append(styleEl)
      mountedStyle = ''
    }
    if (css !== mountedStyle) {
      styleEl.textContent = css
      mountedStyle = css
    }
    // Replacing the body destroys whatever had focus, so a box reached by
    // keyboard gets it back.
    const focused = (target.activeElement as HTMLElement | null)?.dataset?.mdLine
    // Everything after the stylesheet, replaced in one parse.
    while (styleEl.nextSibling) styleEl.nextSibling.remove()
    styleEl.insertAdjacentHTML('afterend', body)
    if (focused) {
      target
        .querySelector<HTMLInputElement>(`input[data-md-line="${focused}"]`)
        ?.focus({ preventScroll: true })
    }
  }

  // The download carries a script for these two behaviours. A fragment does
  // not, and the preview does not want one: making it run would mean lifting
  // a <script> out of the rendered document and executing it at page level,
  // which is a poor thing to hand a renderer whose input is a file someone
  // sent you. Re-implementing the same two behaviours costs less.
  function onClick(e: Event) {
    const clicked = e.composedPath()[0]
    if (!(clicked instanceof Element)) return

    const copy = clicked.closest('.md2pdf-copy')
    if (copy instanceof HTMLElement) {
      void copyCode(copy)
      return
    }

    const link = clicked.closest('a[href^="#"]')
    if (!(link instanceof HTMLAnchorElement) || !root) return
    // The browser cannot resolve a fragment against ids inside a shadow
    // root, so the jump is ours to make.
    e.preventDefault()
    const toggle = root.getElementById('md2pdf-toc-state')
    if (toggle instanceof HTMLInputElement) toggle.checked = false
    const id = decodeURIComponent(link.hash.slice(1))
    const target = root.getElementById(id)
    if (!target) return
    target.scrollIntoView({ block: 'start', behavior: 'smooth' })
    onnavigate?.(id)
  }

  /** Jump to `id` without a click — for a link arriving in the URL. */
  export function scrollTo(id: string): boolean {
    const target = root?.getElementById(id)
    target?.scrollIntoView({ block: 'start' })
    return !!target
  }

  /** Open or close the outline drawer. It is a pure-CSS checkbox. */
  export function toggleOutline() {
    const toggle = root?.getElementById('md2pdf-toc-state')
    if (toggle instanceof HTMLInputElement) toggle.checked = !toggle.checked
  }

  /** Every block that knows its source line, in document order. */
  function anchors(): { line: number; el: HTMLElement }[] {
    return [...(root?.querySelectorAll<HTMLElement>('[data-md-line]') ?? [])]
      .filter((el) => el.tagName !== 'INPUT')
      .map((el) => ({ line: Number(el.dataset.mdLine), el }))
  }

  /** Bring the block covering `line` to the top of the pane. */
  export function scrollToLine(line: number) {
    const all = anchors()
    // The last block at or before the line — the one it is inside.
    let best: HTMLElement | undefined
    for (const a of all) {
      if (a.line > line) break
      best = a.el
    }
    ;(best ?? all[0]?.el)?.scrollIntoView({ block: 'start' })
  }

  /** The source line of the topmost block currently on screen. */
  export function lineAtTop(): number | null {
    const pane = host?.parentElement
    if (!pane) return null
    const top = pane.getBoundingClientRect().top
    let best: number | null = null
    for (const { line, el } of anchors()) {
      // A little tolerance, so the block straddling the edge counts as
      // the one you are looking at.
      if (el.getBoundingClientRect().bottom >= top + 4) {
        best = line
        break
      }
    }
    return best
  }

  function onChange(e: Event) {
    const box = e.composedPath()[0]
    if (!(box instanceof HTMLInputElement) || !box.dataset.mdLine) return
    ontasktoggle?.(Number(box.dataset.mdLine), box.checked)
  }

  async function copyCode(button: HTMLElement) {
    const code = button.parentElement?.querySelector('code')
    if (!code || !navigator.clipboard) return
    await navigator.clipboard.writeText(code.textContent ?? '')
    const was = button.textContent
    button.textContent = button.dataset.done ?? was
    setTimeout(() => {
      button.textContent = was
    }, 1200)
  }

  // The scroller is the pane around the host, not the shadow root.
  $effect(() => {
    const pane = host?.parentElement
    if (!pane || !onscrolled) return
    const notify = () => onscrolled()
    pane.addEventListener('scroll', notify, { passive: true })
    return () => pane.removeEventListener('scroll', notify)
  })

  // `data-theme` on the host wins over `prefers-color-scheme` inside it.
  $effect(() => {
    host?.setAttribute('data-theme', theme)
  })
</script>

<div class="html-preview" bind:this={host}></div>

<style>
  .html-preview {
    display: block;
    min-height: 100%;
    /* The engine's sheet paints the document; this keeps the pane from
		   flashing white behind it while a dark document mounts. */
    background: var(--md-bg, #fff);
  }
</style>
