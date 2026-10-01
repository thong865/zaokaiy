/**
 * Plug-in layer: image suggestions — while a seller types a product name, suggest matching photos
 * already on the platform (stock library, own library, opted-in shops) to add with one click.
 * Nuxt loads every folder in `layers/` automatically — delete this folder to remove the UI.
 * Backend: `backend/src/modules/image_suggest` (switch off with IMAGE_SUGGEST_ENABLED=false).
 */
export default defineNuxtConfig({
  i18n: {
    locales: [
      { code: 'en', files: ['en/imgsuggest.json'] },
      { code: 'lo', files: ['lo/imgsuggest.json'] },
    ],
  },
})
