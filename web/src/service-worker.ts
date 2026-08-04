/// <reference lib="webworker" />

declare const self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST?: Array<{ url: string; revision?: string | null }>
}

const CACHE = 'md2pdf-precache-v1'
const PRECACHE = (self.__WB_MANIFEST ?? []).map(({ url }) => new URL(url, self.location.origin).href)

function isolated(response: Response): Response {
  if (response.type === 'opaque' || response.status === 0) return response
  const headers = new Headers(response.headers)
  headers.set('Cross-Origin-Opener-Policy', 'same-origin')
  headers.set('Cross-Origin-Embedder-Policy', 'require-corp')
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)).then(() => self.skipWaiting()))
})

self.addEventListener('activate', (event) => {
  if (PRECACHE.length === 0) return
  event.waitUntil((async () => {
    await Promise.all((await caches.keys()).filter((key) => key !== CACHE).map((key) => caches.delete(key)))
    const cache = await caches.open(CACHE)
    const keep = new Set(PRECACHE)
    await Promise.all((await cache.keys()).filter((request) => !keep.has(request.url)).map((request) => cache.delete(request)))
    await self.clients.claim()
  })())
})

self.addEventListener('fetch', (event) => {
  if (PRECACHE.length === 0) return
  const request = event.request
  if (request.method !== 'GET' || new URL(request.url).origin !== self.location.origin) return
  event.respondWith((async () => {
    const cache = await caches.open(CACHE)
    const cached = await cache.match(request, { ignoreSearch: true })
    if (cached) return isolated(cached)
    try {
      const response = await fetch(request)
      if (response.ok) await cache.put(request, response.clone())
      return isolated(response)
    } catch (error) {
      if (request.mode === 'navigate') {
        const fallback = await cache.match(new URL('./', self.registration.scope).href)
        if (fallback) return isolated(fallback)
      }
      throw error
    }
  })())
})
