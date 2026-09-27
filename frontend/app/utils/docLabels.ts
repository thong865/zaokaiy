/**
 * Labels for printed documents (A4 invoices, thermal receipts, Z report).
 *
 * Printed documents are English + one local language, picked per document with `docLang(currency)`:
 * Lao when the UI is in Lao or the document is in LAK, Thai for THB, otherwise English only.
 * `th` is only set where the Thai wording was already printed before — entries without `th`
 * stay English-only on Thai documents.
 */
export type DocLang = 'lo' | 'th'
interface DocLabel { en: string; lo: string; th?: string }

export const DOC_LABELS = {
  // ---- titles
  invoice: { en: 'INVOICE', lo: 'ໃບແຈ້ງໜີ້', th: 'ใบแจ้งหนี้' },
  taxInvoice: { en: 'TAX INVOICE', lo: 'ໃບເກັບເງິນອາກອນ', th: 'ใบกำกับภาษี' },
  invoiceTaxInvoice: { en: 'INVOICE / TAX INVOICE', lo: 'ໃບແຈ້ງໜີ້/ໃບເກັບເງິນອາກອນ', th: 'ใบแจ้งหนี้/ใบกำกับภาษี' },
  invoiceReceipt: { en: 'INVOICE / RECEIPT', lo: 'ໃບແຈ້ງໜີ້/ໃບຮັບເງິນ', th: 'ใบแจ้งหนี้/ใบเสร็จรับเงิน' },
  packingSlip: { en: 'PACKING SLIP', lo: 'ໃບລາຍການສິນຄ້າ', th: 'ใบรายการสินค้า' },
  receipt: { en: 'RECEIPT', lo: 'ໃບຮັບເງິນ', th: 'ใบเสร็จรับเงิน' },
  receiptAbb: { en: 'RECEIPT / TAX INVOICE (ABB)', lo: 'ໃບຮັບເງິນ/ໃບເກັບເງິນອາກອນແບບຫຍໍ້', th: 'ใบเสร็จรับเงิน/ใบกำกับภาษีอย่างย่อ' },
  zReport: { en: 'END OF DAY REPORT (Z)', lo: 'ລາຍງານສະຫຼຸບທ້າຍວັນ (Z)' },
  original: { en: 'ORIGINAL', lo: 'ຕົ້ນສະບັບ', th: 'ต้นฉบับ' },
  copy: { en: 'COPY', lo: 'ສຳເນົາ', th: 'สำเนา' },
  void: { en: 'VOID', lo: 'ຍົກເລີກ' },
  voided: { en: 'VOIDED', lo: 'ຍົກເລີກແລ້ວ' },

  // ---- header / parties
  no: { en: 'No.', lo: 'ເລກທີ', th: 'เลขที่' },
  date: { en: 'Date', lo: 'ວັນທີ', th: 'วันที่' },
  tel: { en: 'Tel.', lo: 'ໂທ' },
  taxId: { en: 'Tax ID', lo: 'ເລກປະຈຳຕົວຜູ້ເສຍອາກອນ', th: 'เลขประจำตัวผู้เสียภาษี' },
  buyerTaxId: { en: 'Tax ID', lo: 'ເລກປະຈຳຕົວຜູ້ເສຍອາກອນ' },
  customer: { en: 'Customer', lo: 'ລູກຄ້າ', th: 'ลูกค้า' },
  receiptRef: { en: 'Receipt', lo: 'ໃບຮັບເງິນ', th: 'ใบเสร็จ' },
  status: { en: 'Status', lo: 'ສະຖານະ' },
  channel: { en: 'Channel', lo: 'ຊ່ອງທາງ' },
  cashier: { en: 'Cashier', lo: 'ພະນັກງານເກັບເງິນ' },

  // ---- lines
  description: { en: 'Description', lo: 'ລາຍການ', th: 'รายการ' },
  qty: { en: 'Qty', lo: 'ຈຳນວນ' },
  unitPrice: { en: 'Unit price', lo: 'ລາຄາຕໍ່ໜ່ວຍ' },
  colDiscount: { en: 'Discount', lo: 'ສ່ວນຫຼຸດ' },
  amount: { en: 'Amount', lo: 'ຈຳນວນເງິນ' },
  shipping: { en: 'Shipping', lo: 'ຄ່າຈັດສົ່ງ', th: 'ค่าจัดส่ง' },
  items: { en: 'Items', lo: 'ຈຳນວນສິນຄ້າ' },

  // ---- totals
  subtotal: { en: 'Subtotal', lo: 'ລວມຍ່ອຍ', th: 'รวมเงิน' },
  discount: { en: 'Discount', lo: 'ສ່ວນຫຼຸດ', th: 'ส่วนลด' },
  beforeVat: { en: 'Amount before VAT', lo: 'ມູນຄ່າກ່ອນອາກອນ', th: 'มูลค่าก่อนภาษี' },
  beforeVatShort: { en: 'Before VAT', lo: 'ກ່ອນ ອມພ' },
  vat: { en: 'VAT', lo: 'ອາກອນມູນຄ່າເພີ່ມ', th: 'ภาษีมูลค่าเพิ่ม' },
  vatShort: { en: 'VAT', lo: 'ອມພ' },
  vatIncl: { en: '(incl.)', lo: '(ລວມແລ້ວ)' },
  vatIncluded: { en: 'VAT included', lo: 'ລວມ ອມພ ແລ້ວ' },
  grandTotal: { en: 'Grand total', lo: 'ຍອດລວມສຸດທິ', th: 'จำนวนเงินทั้งสิ้น' },
  total: { en: 'TOTAL', lo: 'ລວມທັງໝົດ' },
  paid: { en: 'Paid', lo: 'ຊຳລະແລ້ວ' },
  change: { en: 'Change', lo: 'ເງິນທອນ', th: 'เงินทอน' },

  // ---- payment methods (labels only; API values stay as-is)
  pay_cash: { en: 'Cash', lo: 'ເງິນສົດ', th: 'เงินสด' },
  pay_card: { en: 'Card', lo: 'ບັດ', th: 'บัตร' },
  pay_transfer: { en: 'Transfer', lo: 'ໂອນ', th: 'โอน' },
  pay_qr: { en: 'QR', lo: 'ຊຳລະຜ່ານ QR', th: 'PromptPay' },
  pay_other: { en: 'Other', lo: 'ອື່ນໆ' },

  // ---- footer / notes
  receivedBy: { en: 'Received by', lo: 'ຜູ້ຮັບສິນຄ້າ', th: 'ผู้รับสินค้า' },
  authorizedSignature: { en: 'Authorized signature', lo: 'ຜູ້ມີອຳນາດລົງນາມ', th: 'ผู้มีอำนาจลงนาม' },
  signature: { en: 'Signature', lo: 'ລາຍເຊັນ' },
  fullTaxInvoice: { en: 'Full tax invoice', lo: 'ໃບເກັບເງິນອາກອນເຕັມຮູບແບບ' },
  awaitingPayment: { en: 'Awaiting payment.', lo: 'ລໍຖ້າການຊຳລະເງິນ' },
  customerNote: { en: 'Customer note', lo: 'ໝາຍເຫດຈາກລູກຄ້າ' },
  tracking: { en: 'Tracking', lo: 'ເລກພັດສະດຸ' },
  thankYou: { en: 'Thank you', lo: 'ຂອບໃຈທີ່ອຸດໜູນ', th: 'ขอบคุณ' },
  printed: { en: 'Printed', lo: 'ພິມເມື່ອ' },

  // ---- Z report
  zSales: { en: 'Sales', lo: 'ຈຳນວນບິນ' },
  zReceipts: { en: 'Receipts', lo: 'ເລກບິນ' },
  zTaxInvoices: { en: 'Tax invoices', lo: 'ໃບເກັບເງິນອາກອນ' },
  zGross: { en: 'Gross sales', lo: 'ຍອດຂາຍລວມ' },
  zBillDiscounts: { en: 'Bill discounts', lo: 'ສ່ວນຫຼຸດທ້າຍບິນ' },
  zByMethod: { en: 'Takings by method', lo: 'ຍອດຮັບຕາມວິທີຊຳລະ' },
  zVoided: { en: 'Voided sales', lo: 'ບິນທີ່ຍົກເລີກ' },
  zTopItems: { en: 'Top items', lo: 'ສິນຄ້າຂາຍດີ' },
} satisfies Record<string, DocLabel>

export type DocKey = keyof typeof DOC_LABELS

const get = (key: DocKey | (string & {})): DocLabel | undefined => (DOC_LABELS as Record<string, DocLabel>)[key]

/** Local language of a printed document: Lao if the UI is Lao or the amount is in kip, Thai for baht, else none. */
export function docLang(currency: string | null | undefined): DocLang | null {
  if (intlLocale() === 'lo-LA' || currency === 'LAK') return 'lo'
  if (currency === 'THB') return 'th'
  return null
}

/** English text of a label. Unknown keys are returned as-is. */
export const dlEn = (key: DocKey | (string & {})): string => get(key)?.en ?? key

/** Local text only (falls back to English when there is no local wording / no local language). */
export function dlLocal(key: DocKey | (string & {}), lang: DocLang | null): string {
  const l = get(key)
  if (!l) return key
  return (lang && l[lang]) || l.en
}

/** "English / Local" — or just English when there is no local wording. */
export function dl(key: DocKey | (string & {}), lang: DocLang | null): string {
  const l = get(key)
  if (!l) return key
  const local = lang ? l[lang] : undefined
  return local ? `${l.en} / ${local}` : l.en
}

/** "Local / English" (local first) — or just English. */
export function dlr(key: DocKey | (string & {}), lang: DocLang | null): string {
  const l = get(key)
  if (!l) return key
  const local = lang ? l[lang] : undefined
  return local ? `${local} / ${l.en}` : l.en
}

/** Intl locale for dates on a document. */
export const docLocale = (lang: DocLang | null) => (lang === 'lo' ? 'lo-LA' : 'en-GB')
