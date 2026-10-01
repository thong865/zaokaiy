/** Amount-in-words for invoices: Thai baht text (บาทถ้วน), Lao kip text (ກີບຖ້ວນ) and English. */

const TH_DIGITS = ['ศูนย์', 'หนึ่ง', 'สอง', 'สาม', 'สี่', 'ห้า', 'หก', 'เจ็ด', 'แปด', 'เก้า']
const TH_PLACES = ['', 'สิบ', 'ร้อย', 'พัน', 'หมื่น', 'แสน']

/** Reads 0..999,999 in Thai. `edHint` = a higher group exists, so a lone trailing 1 reads "เอ็ด". */
function thGroup(n: number, edHint: boolean): string {
  const digits = String(n).split('').map(Number)
  const len = digits.length
  let out = ''
  digits.forEach((d, i) => {
    const place = len - 1 - i
    if (d === 0) return
    if (place === 0 && d === 1 && (len > 1 || edHint)) out += 'เอ็ด'
    else if (place === 1 && d === 1) out += 'สิบ'
    else if (place === 1 && d === 2) out += 'ยี่สิบ'
    else out += TH_DIGITS[d]! + TH_PLACES[place]!
  })
  return out
}

function thNumber(n: number): string {
  if (n === 0) return TH_DIGITS[0]!
  const parts: number[] = []
  while (n > 0) {
    parts.unshift(n % 1_000_000)
    n = Math.floor(n / 1_000_000)
  }
  return parts
    .map((p, i) => (p ? thGroup(p, i > 0) : '') + (i < parts.length - 1 ? 'ล้าน' : ''))
    .join('')
}

export function bahtText(cents: number): string {
  const neg = cents < 0
  const abs = Math.abs(Math.round(cents))
  const baht = Math.floor(abs / 100)
  const satang = abs % 100
  let out = ''
  if (baht > 0 || satang === 0) out += thNumber(baht) + 'บาท'
  out += satang === 0 ? 'ถ้วน' : thNumber(satang) + 'สตางค์'
  return (neg ? 'ลบ' : '') + out
}

const LO_DIGITS = ['ສູນ', 'ໜຶ່ງ', 'ສອງ', 'ສາມ', 'ສີ່', 'ຫ້າ', 'ຫົກ', 'ເຈັດ', 'ແປດ', 'ເກົ້າ']
const LO_PLACES = ['', 'ສິບ', 'ຮ້ອຍ', 'ພັນ', 'ໝື່ນ', 'ແສນ']

/** Reads 0..999,999 in Lao. `edHint` = a higher group exists, so a lone trailing 1 reads "ເອັດ". */
function loGroup(n: number, edHint: boolean): string {
  const digits = String(n).split('').map(Number)
  const len = digits.length
  let out = ''
  digits.forEach((d, i) => {
    const place = len - 1 - i
    if (d === 0) return
    if (place === 0 && d === 1 && (len > 1 || edHint)) out += 'ເອັດ'
    else if (place === 1 && d === 1) out += 'ສິບ'
    else if (place === 1 && d === 2) out += 'ຊາວ'
    else out += LO_DIGITS[d]! + LO_PLACES[place]!
  })
  return out
}

function loNumber(n: number): string {
  if (n === 0) return LO_DIGITS[0]!
  const parts: number[] = []
  while (n > 0) {
    parts.unshift(n % 1_000_000)
    n = Math.floor(n / 1_000_000)
  }
  return parts
    .map((p, i) => (p ? loGroup(p, i > 0) : '') + (i < parts.length - 1 ? 'ລ້ານ' : ''))
    .join('')
}

/** Lao kip text, e.g. 12_500_000 cents → "ໜຶ່ງແສນສອງໝື່ນຫ້າພັນກີບຖ້ວນ". */
export function kipText(cents: number): string {
  const neg = cents < 0
  const abs = Math.abs(Math.round(cents))
  const kip = Math.floor(abs / 100)
  const att = abs % 100
  let out = ''
  if (kip > 0 || att === 0) out += loNumber(kip) + 'ກີບ'
  out += att === 0 ? 'ຖ້ວນ' : loNumber(att) + 'ອັດ'
  return (neg ? 'ລົບ' : '') + out
}

const ONES = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten', 'eleven', 'twelve',
  'thirteen', 'fourteen', 'fifteen', 'sixteen', 'seventeen', 'eighteen', 'nineteen']
const TENS = ['', '', 'twenty', 'thirty', 'forty', 'fifty', 'sixty', 'seventy', 'eighty', 'ninety']
const SCALES = ['', 'thousand', 'million', 'billion', 'trillion']

function en999(n: number): string {
  const h = Math.floor(n / 100)
  const r = n % 100
  const parts: string[] = []
  if (h) parts.push(`${ONES[h]} hundred`)
  if (r) parts.push(r < 20 ? ONES[r]! : TENS[Math.floor(r / 10)]! + (r % 10 ? `-${ONES[r % 10]}` : ''))
  return parts.join(' ')
}

export function englishNumber(n: number): string {
  if (n === 0) return 'zero'
  const groups: string[] = []
  let i = 0
  while (n > 0) {
    const g = n % 1000
    if (g) groups.unshift(en999(g) + (SCALES[i] ? ` ${SCALES[i]}` : ''))
    n = Math.floor(n / 1000)
    i++
  }
  return groups.join(' ')
}

const CURRENCY_WORDS: Record<string, [string, string]> = {
  THB: ['baht', 'satang'],
  LAK: ['kip', 'att'],
  USD: ['dollars', 'cents'],
  VND: ['dong', 'xu'],
}

export function amountInEnglish(cents: number, currency = 'THB'): string {
  const [major, minor] = CURRENCY_WORDS[currency] ?? [currency, 'cents']
  const abs = Math.abs(Math.round(cents))
  const whole = Math.floor(abs / 100)
  const frac = abs % 100
  const text = `${englishNumber(whole)} ${major}` + (frac ? ` and ${englishNumber(frac)} ${minor}` : ' only')
  return text.charAt(0).toUpperCase() + text.slice(1)
}
