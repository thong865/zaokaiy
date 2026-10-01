<script setup lang="ts">
/**
 * Scan barcodes with the phone/tablet camera. Uses the browser's built-in BarcodeDetector when
 * available (Chrome, Edge, Android) and otherwise a bundled ZXing WebAssembly reader (Safari,
 * Firefox) — loaded only when the camera is opened, and served from this site (works offline).
 */
const props = withDefaults(defineProps<{ continuous?: boolean }>(), { continuous: true })
const emit = defineEmits<{ detected: [code: string]; close: [] }>()
const { t } = useI18n()

const video = ref<HTMLVideoElement | null>(null)
const error = ref('')
const starting = ref(true)
const last = ref('')
const torch = ref(false)
const torchSupported = ref(false)
let stream: MediaStream | null = null
let raf = 0
let stopped = false
let lastCode = ''
/** Last time `lastCode` was seen in the frame. The same code is accepted again only after it has
 *  left the view (e.g. the next identical item), never while one item just stays in front of the lens. */
let lastSeen = 0
const GONE_MS = 900

const FORMATS = ['ean_13', 'ean_8', 'upc_a', 'upc_e', 'code_128', 'code_39', 'code_93', 'itf', 'codabar', 'qr_code', 'data_matrix']

interface Detector { detect(src: CanvasImageSource): Promise<{ rawValue: string }[]> }

async function makeDetector(): Promise<Detector> {
  const Native = (globalThis as unknown as { BarcodeDetector?: { new (o: { formats: string[] }): Detector; getSupportedFormats(): Promise<string[]> } }).BarcodeDetector
  if (Native) {
    try {
      const supported = await Native.getSupportedFormats()
      const formats = FORMATS.filter((f) => supported.includes(f))
      if (formats.length) return new Native({ formats })
    } catch {}
  }
  const [{ BarcodeDetector, setZXingModuleOverrides }, { default: wasmUrl }] = await Promise.all([
    import('barcode-detector/ponyfill'),
    import('zxing-wasm/reader/zxing_reader.wasm?url'),
  ])
  setZXingModuleOverrides({ locateFile: (path: string, prefix: string) => (path.endsWith('.wasm') ? wasmUrl : prefix + path) })
  return new BarcodeDetector({ formats: FORMATS as never }) as unknown as Detector
}

async function start() {
  error.value = ''
  starting.value = true
  try {
    if (!navigator.mediaDevices?.getUserMedia) throw new Error(t('pos.scan.cameraUnsupported'))
    stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: { ideal: 'environment' }, width: { ideal: 1280 }, height: { ideal: 720 } }, audio: false })
    const track = stream.getVideoTracks()[0]
    torchSupported.value = !!(track?.getCapabilities?.() as { torch?: boolean } | undefined)?.torch
    if (!video.value) return
    video.value.srcObject = stream
    await video.value.play()
    const detector = await makeDetector()
    starting.value = false
    let busy = false
    let lastRun = 0
    const loop = async (ts: number) => {
      if (stopped) return
      raf = requestAnimationFrame(loop)
      if (busy || ts - lastRun < 120 || !video.value || video.value.readyState < 2) return
      busy = true
      lastRun = ts
      try {
        const found = await detector.detect(video.value)
        const code = found[0]?.rawValue?.trim()
        const now = Date.now()
        if (code && code === lastCode && now - lastSeen < GONE_MS) {
          lastSeen = now // still the same item in view
        } else if (code) {
          lastCode = code
          lastSeen = now
          last.value = code
          navigator.vibrate?.(40)
          emit('detected', code)
          if (!props.continuous) close()
        }
      } catch {}
      busy = false
    }
    raf = requestAnimationFrame(loop)
  } catch (e) {
    starting.value = false
    const name = (e as { name?: string }).name
    error.value = name === 'NotAllowedError' ? t('pos.scan.cameraDenied') : name === 'NotFoundError' ? t('pos.scan.noCamera') : (e as Error).message || t('pos.scan.cameraUnsupported')
  }
}

async function toggleTorch() {
  const track = stream?.getVideoTracks()[0]
  if (!track) return
  torch.value = !torch.value
  try {
    await track.applyConstraints({ advanced: [{ torch: torch.value } as MediaTrackConstraintSet] })
  } catch {
    torch.value = false
  }
}

function stop() {
  stopped = true
  cancelAnimationFrame(raf)
  stream?.getTracks().forEach((tr) => tr.stop())
  stream = null
}
function close() {
  stop()
  emit('close')
}
onMounted(start)
onBeforeUnmount(stop)
</script>

<template>
  <div class="fixed inset-0 z-50 flex flex-col bg-night/95 text-white" role="dialog" :aria-label="$t('pos.scan.camera')">
    <div class="flex items-center justify-between p-4">
      <div class="font-bold">{{ $t('pos.scan.camera') }}</div>
      <div class="flex gap-2">
        <button v-if="torchSupported" type="button" class="rounded-full bg-white/10 px-3 py-1.5 text-sm font-semibold" @click="toggleTorch">{{ torch ? $t('pos.scan.torchOff') : $t('pos.scan.torchOn') }}</button>
        <button type="button" class="rounded-full bg-surface px-3 py-1.5 text-sm font-bold text-ink" @click="close">{{ $t('common.close') }}</button>
      </div>
    </div>
    <div class="relative mx-auto aspect-[4/3] w-full max-w-2xl overflow-hidden bg-black">
      <video ref="video" class="size-full object-cover" muted playsinline />
      <div class="pointer-events-none absolute inset-x-8 top-1/2 h-32 -translate-y-1/2 rounded-2xl border-2 border-white/80 shadow-[0_0_0_9999px_rgba(0,0,0,0.35)]">
        <div class="absolute inset-x-3 top-1/2 h-0.5 animate-pulse bg-brand-500" />
      </div>
      <div v-if="starting && !error" class="absolute inset-0 grid place-items-center text-sm text-white/80">{{ $t('pos.scan.starting') }}</div>
    </div>
    <div class="mx-auto w-full max-w-2xl p-4 text-center text-sm">
      <p v-if="error" class="rounded-xl bg-brand-500/20 p-3 text-brand-100">{{ error }}</p>
      <p v-else-if="last" class="font-mono text-lg">✓ {{ last }}</p>
      <p v-else class="text-white/70">{{ $t('pos.scan.aim') }}</p>
    </div>
  </div>
</template>
