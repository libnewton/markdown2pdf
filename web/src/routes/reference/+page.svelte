<script lang="ts">
  import PdfEditor from '$lib/components/PdfEditor.svelte'
  import { PDF_TEMPLATES } from '$lib/templates/pdf-templates'
</script>

<svelte:head>
  <link rel="canonical" href="/reference" />
</svelte:head>

<!-- The feature demo, source next to result. Seeded content never touches
     IndexedDB, so reading the reference cannot disturb your own documents. -->
<PdfEditor
  seoTitle="md2pdf reference · every Markdown feature, side by side"
  seoDescription="The md2pdf feature demo: every supported Markdown construct, frontmatter key and layout option, with the rendered result beside the source."
  initialMarkdown={PDF_TEMPLATES[0].content}
  readOnly
/>
