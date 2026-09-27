# i18n conventions (English / Lao)

- One JSON file per namespace in `i18n/locales/en/` and `i18n/locales/lo/`; the file holds ONE top-level key equal to the file name
  (`pos.json` → `{ "pos": { … } }`). Files are picked up automatically by `nuxt.config.ts`.
- `en` and `lo` must have identical key sets (`node scripts/i18n-check.mjs`).
- In templates use `$t('ns.key')`; in `<script setup>` use `const { t } = useI18n()`. Outside components use `tr('ns.key')` (utils/format.ts).
  Anything built in script that contains text must be `computed` so it re-renders when the language changes.
- Placeholders: `{name}` — `$t('pos.noMatch', { code })`. Plurals (English only): `"{n} item | {n} items"` + `$t(key, n)` / `t(key, { n }, n)`.
  Lao has no plural forms — write one form.
- Rich text: `<i18n-t keypath="…" tag="p"><template #link><NuxtLink …>{{ $t('…') }}</NuxtLink></template></i18n-t>`.
- Formatting (utils/format.ts, locale-aware): `money()`, `num()`, `ago()`, `fmtDate()`, `fmtDateTime()`, `fmtTime()`, `fmtDayMonth()`, `fmtWeekdayTime()`, `fmtDocDate()`. Never call `toLocaleString` directly.
- Shared words live in `common` (see `common.json`: actions, nav, `common.status.*`, `common.pay.*`, `common.kind.*`, `common.review.*`).
- Don't translate: brand `zaokaiy`, product/shop names, user data, SKU/code values, currency codes, API values sent to the backend.
- Keep digits Latin (0-9) in Lao. Numbers use ',' grouping in both languages. Lao dates are formatted by our own code in utils/format.ts, because Chrome has no Lao Intl data; never format dates with Intl directly.

## Lao glossary (use these terms consistently)

| English | Lao |
|---|---|
| shop / store | ຮ້ານ |
| storefront | ໜ້າຮ້ານ |
| marketplace | ຕະຫຼາດ |
| product | ສິນຄ້າ |
| category / sub-category | ໝວດໝູ່ / ໝວດໝູ່ຍ່ອຍ |
| order | ຄຳສັ່ງຊື້ |
| cart | ກະຕ່າ |
| checkout | ຊຳລະເງິນ |
| stock | ສະຕັອກ |
| inventory | ສາງສິນຄ້າ |
| price / discount | ລາຄາ / ສ່ວນຫຼຸດ |
| subtotal / total / grand total | ລວມຍ່ອຍ / ລວມທັງໝົດ / ຍອດລວມສຸດທິ |
| VAT | ອາກອນມູນຄ່າເພີ່ມ (ອມພ) |
| invoice / tax invoice | ໃບແຈ້ງໜີ້ / ໃບເກັບເງິນອາກອນ |
| receipt / bill | ໃບຮັບເງິນ / ໃບບິນ |
| packing slip | ໃບລາຍການສິນຄ້າ |
| payment / pay | ການຊຳລະເງິນ / ຊຳລະ |
| cash / bank transfer / card / change | ເງິນສົດ / ໂອນຜ່ານທະນາຄານ / ບັດ / ເງິນທອນ |
| shipping / delivery | ການຈັດສົ່ງ / ຄ່າຈັດສົ່ງ |
| seller / buyer / customer | ຜູ້ຂາຍ / ຜູ້ຊື້ / ລູກຄ້າ |
| sell-staff (reseller) | ຕົວແທນຂາຍ |
| commission | ຄ່ານາຍໜ້າ |
| creator | ຄຣີເອເຕີ |
| content / post | ຄອນເທັນ / ໂພສ |
| ads / campaign / budget | ໂຄສະນາ / ແຄມເປນ / ງົບປະມານ |
| media library / image / video | ຄັງສື່ / ຮູບພາບ / ວິດີໂອ |
| dashboard | ໜ້າຈັດການ |
| settings | ການຕັ້ງຄ່າ |
| review (approval) / approve / reject | ການກວດ / ອະນຸມັດ / ປະຕິເສດ |
| draft / publish | ສະບັບຮ່າງ / ເຜີຍແຜ່ |
| live (stream) / comment | ໄລຟ໌ / ຄອມເມັ້ນ |
| channel | ຊ່ອງທາງ |
| reserve (stock) | ຈອງ |
| admin | ຜູ້ດູແລລະບົບ |
| AI assistant | ຜູ້ຊ່ວຍ AI |
| log in / log out / register | ເຂົ້າສູ່ລະບົບ / ອອກຈາກລະບົບ / ລົງທະບຽນ |
| save / cancel / delete / edit | ບັນທຶກ / ຍົກເລີກ / ລຶບ / ແກ້ໄຂ |
| search / upload / print | ຄົ້ນຫາ / ອັບໂຫຼດ / ພິມ |

Tone: polite, short, natural Lao as used in Lao apps (not Thai). Prefer Lao spelling (ໃຫ້, ໄດ້, ບໍ່, ແລ້ວ, ຂໍ້ມູນ, ເງິນ).
