import type { MediaAsset } from '~/utils/types'

export const ACCEPT = 'image/jpeg,image/png,image/webp,image/gif,video/mp4,video/webm,video/quicktime'
const IMAGE_MB = 15
const VIDEO_MB = 200

export interface UploadJob {
  id: string
  file: File
  preview: string | null
  progress: number
  status: 'queued' | 'uploading' | 'done' | 'error'
  error?: string
  asset?: MediaAsset
}

export interface MediaConfig {
  max_image_mb: number
  max_video_mb: number
  /** Minimum px on the shortest side (the API rejects smaller images). */
  min_image_edge: number
  recommended_edge: number
  variant_widths: number[]
}

/** Upload limits and image guidance from the API. */
export function useMediaConfig() {
  const api = useApi()
  return useAsyncData('media-config', () => api<MediaConfig>('/media/config'), {
    default: (): MediaConfig => ({ max_image_mb: IMAGE_MB, max_video_mb: VIDEO_MB, min_image_edge: 500, recommended_edge: 1200, variant_widths: [] }),
  })
}

/** Media library API helpers. Uploads use XHR so we can show per-file progress. */
export function useMedia() {
  const config = useRuntimeConfig()
  const token = useCookie<string | null>('zk_token')
  const api = useApi()

  function validate(file: File): string | null {
    const isVideo = file.type.startsWith('video/')
    const isImage = file.type.startsWith('image/')
    if (!isVideo && !isImage) return tr('media.err.unsupported')
    const limit = (isVideo ? VIDEO_MB : IMAGE_MB) * 1024 * 1024
    if (file.size > limit) return tr('media.err.tooLarge', { mb: isVideo ? VIDEO_MB : IMAGE_MB })
    return null
  }

  function makeJobs(files: FileList | File[]): UploadJob[] {
    return Array.from(files).map((file) => {
      const error = validate(file)
      return {
        id: crypto.randomUUID(),
        file,
        preview: file.type.startsWith('image/') ? URL.createObjectURL(file) : null,
        progress: 0,
        status: error ? 'error' : 'queued',
        error: error ?? undefined,
      }
    })
  }

  function uploadOne(shopId: string, job: UploadJob, productId?: string): Promise<MediaAsset> {
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      const q = productId ? `?product_id=${productId}` : ''
      xhr.open('POST', `${config.public.apiBase}/shops/${shopId}/media/upload${q}`)
      if (token.value) xhr.setRequestHeader('Authorization', `Bearer ${token.value}`)
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) job.progress = Math.round((e.loaded / e.total) * 95)
      }
      xhr.onload = () => {
        let body: { assets?: MediaAsset[]; errors?: { message: string }[]; message?: string } = {}
        try {
          body = JSON.parse(xhr.responseText)
        } catch {}
        if (xhr.status >= 200 && xhr.status < 300 && body.assets?.length) {
          job.progress = 100
          resolve(body.assets[0]!)
        } else {
          reject(new Error(body.errors?.[0]?.message || body.message || tr('media.err.uploadFailed', { status: xhr.status })))
        }
      }
      xhr.onerror = () => reject(new Error(tr('media.err.network')))
      const fd = new FormData()
      fd.append('file', job.file, job.file.name)
      xhr.send(fd)
    })
  }

  /** Uploads queued jobs (3 at a time). Jobs are mutated in place for reactive progress. */
  async function runJobs(shopId: string, jobs: UploadJob[], productId?: string): Promise<MediaAsset[]> {
    const queue = jobs.filter((j) => j.status === 'queued')
    const done: MediaAsset[] = []
    const worker = async () => {
      for (let job = queue.shift(); job; job = queue.shift()) {
        job.status = 'uploading'
        try {
          job.asset = await uploadOne(shopId, job, productId)
          job.status = 'done'
          done.push(job.asset)
        } catch (e) {
          job.status = 'error'
          job.error = (e as Error).message
        }
      }
    }
    await Promise.all([worker(), worker(), worker()])
    // keep original order
    return jobs.filter((j) => j.asset).map((j) => j.asset!)
  }

  const setGallery = (productId: string, assetIds: string[]) =>
    api<MediaAsset[]>(`/products/${productId}/media`, { method: 'PUT', body: { asset_ids: assetIds } })

  const addUrl = (shopId: string, url: string, alt = '') =>
    api<MediaAsset>(`/shops/${shopId}/media/url`, { method: 'POST', body: { url, alt } })

  /** Reject images below the minimum resolution before uploading (instant feedback, no wasted upload). */
  async function checkResolution(jobs: UploadJob[], minEdge: number) {
    if (!minEdge || typeof createImageBitmap !== 'function') return
    await Promise.all(
      jobs
        .filter((j) => j.status === 'queued' && j.file.type.startsWith('image/'))
        .map(async (j) => {
          try {
            const bmp = await createImageBitmap(j.file)
            const { width, height } = bmp
            bmp.close()
            if (Math.min(width, height) < minEdge) {
              j.status = 'error'
              j.error = tr('media.quality.tooSmall', { w: width, h: height, min: minEdge })
            }
          } catch {
            // Undecodable here (e.g. HEIC); let the server decide.
          }
        }),
    )
  }

  return { makeJobs, runJobs, setGallery, addUrl, validate, checkResolution }
}
