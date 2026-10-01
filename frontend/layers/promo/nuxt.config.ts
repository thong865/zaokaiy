/**
 * Plug-in layer: promotions & loyalty points — platform sign-up campaigns (Facebook / WhatsApp /
 * Google / email), shop campaigns (first order, live codes), per-shop points, and the coupon/points
 * boxes in the cart, the chat-order checkout and the POS.
 * Nuxt loads every folder in `layers/` automatically — delete this folder to remove the UI.
 * Backend: `backend/crates/commerce/src/modules/promo` (switch off with PROMO_ENABLED=false).
 */
export default defineNuxtConfig({
  i18n: {
    locales: [
      { code: 'en', files: ['en/promo.json'] },
      { code: 'lo', files: ['lo/promo.json'] },
    ],
  },
})
