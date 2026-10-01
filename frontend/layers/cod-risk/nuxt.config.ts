/**
 * Plug-in layer: COD risk (report customers who refuse cash-on-delivery parcels).
 * Nuxt picks up every folder in `layers/` automatically — delete this folder to remove the UI.
 * The backend side is `backend/src/modules/cod_risk` (switch off with COD_RISK_ENABLED=false;
 * the nav entries then disappear because GET /api/modules reports it disabled).
 */
export default defineNuxtConfig({
  i18n: {
    locales: [
      { code: 'en', files: ['en/codrisk.json'] },
      { code: 'lo', files: ['lo/codrisk.json'] },
    ],
  },
})
