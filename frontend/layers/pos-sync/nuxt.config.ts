/**
 * Plug-in layer: POS terminals (the zaokaiy POS desktop app for Windows / macOS, `pos-desktop/`).
 * Adds the page where a signed-in owner or cashier approves a terminal's pairing code
 * (`/pos-pair?code=…`) and the owner's list of terminals (`/dashboard/pos-devices`).
 * Backend: `backend/src/modules/pos_sync` (switch off with POS_SYNC_ENABLED=false).
 */
export default defineNuxtConfig({
  i18n: {
    locales: [
      { code: 'en', files: ['en/possync.json'] },
      { code: 'lo', files: ['lo/possync.json'] },
    ],
  },
})
