<script lang="ts">
  let { label }: { label: string } = $props()
</script>

<div class="status-hint" aria-live="polite">
  <div class="status-hint-spinner"></div>
  <span>{label}</span>
</div>

<style>
  /* It floats over the preview, which follows the theme, so it has to as
     well — a slab of near-black over a white page reads as an error. */
  .status-hint {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 999px;
    border: 1px solid var(--color-gray-300);
    background: color-mix(in srgb, var(--color-white) 82%, transparent);
    backdrop-filter: blur(10px);
    box-shadow: var(--shadow-md);
    color: var(--color-gray-700);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    pointer-events: none;
    animation: status-hint-fade-in 0.22s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .status-hint-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--color-gray-300);
    border-top-color: var(--color-gray-700);
    border-radius: 50%;
    animation: status-hint-spin 0.8s linear infinite;
  }

  @keyframes status-hint-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes status-hint-fade-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
</style>
