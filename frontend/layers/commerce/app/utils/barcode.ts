/**
 * Barcode helpers (no dependencies): GS1 check digits and bar patterns for EAN-13 / EAN-8 /
 * UPC-A and Code 128 (B, or C for even-length digit strings), rendered by <BarcodeSvg>.
 * A pattern is a string of modules: "1" = bar, "0" = space.
 */

/** GS1 mod-10 check digit for the digits without the check digit. */
export function gtinCheckDigit(body: string): number | null {
  if (!/^\d+$/.test(body)) return null
  let sum = 0
  for (let i = 0; i < body.length; i++) sum += Number(body[body.length - 1 - i]) * (i % 2 === 0 ? 3 : 1)
  return (10 - (sum % 10)) % 10
}

/** All-digit GTIN-8/12/13/14 with a correct check digit. */
export function gtinValid(code: string): boolean {
  return /^(\d{8}|\d{12,14})$/.test(code) && gtinCheckDigit(code.slice(0, -1)) === Number(code.slice(-1))
}

const L = ['0001101', '0011001', '0010011', '0111101', '0100011', '0110001', '0101111', '0111011', '0110111', '0001011']
const G = ['0100111', '0110011', '0011011', '0100001', '0011101', '0111001', '0000101', '0010001', '0001001', '0010111']
const R = ['1110010', '1100110', '1101100', '1000010', '1011100', '1001110', '1010000', '1000100', '1001000', '1110100']
const PARITY = ['LLLLLL', 'LLGLGG', 'LLGGLG', 'LLGGGL', 'LGLLGG', 'LGGLLG', 'LGGGLL', 'LGLGLG', 'LGLGGL', 'LGGLGL']

function ean13(code: string): string {
  const d = code.split('').map(Number)
  const parity = PARITY[d[0]!]!
  let s = '101'
  for (let i = 1; i <= 6; i++) s += (parity[i - 1] === 'L' ? L : G)[d[i]!]
  s += '01010'
  for (let i = 7; i <= 12; i++) s += R[d[i]!]
  return s + '101'
}

function ean8(code: string): string {
  const d = code.split('').map(Number)
  let s = '101'
  for (let i = 0; i < 4; i++) s += L[d[i]!]
  s += '01010'
  for (let i = 4; i < 8; i++) s += R[d[i]!]
  return s + '101'
}

// Code 128 bar/space widths for values 0..106 (106 = stop, 7 elements).
const C128 = (
  '212222 222122 222221 121223 121322 131222 122213 122312 132212 221213 221312 231212 112232 122132 122231 113222 ' +
  '123122 123221 223211 221132 221231 213212 223112 312131 311222 321122 321221 312212 322112 322211 212123 212321 ' +
  '232121 111323 131123 131321 112313 132113 132311 211313 231113 231311 112133 112331 132131 113123 113321 133121 ' +
  '313121 211331 231131 213113 213311 213131 311123 311321 331121 312113 312311 332111 314111 221411 431111 111224 ' +
  '111422 121124 121421 141122 141221 112214 112412 122114 122411 142112 142211 241211 221114 413111 241112 134111 ' +
  '111242 121142 121241 114212 124112 124211 411212 421112 421211 212141 214121 412121 111143 111341 131141 114113 ' +
  '114311 411113 411311 113141 114131 311141 411131 211412 211214 211232 2331112'
).split(' ')

function widthsToModules(w: string): string {
  let s = ''
  for (let i = 0; i < w.length; i++) s += (i % 2 === 0 ? '1' : '0').repeat(Number(w[i]))
  return s
}

function code128(text: string): string {
  const values: number[] = []
  if (/^\d{4,}$/.test(text) && text.length % 2 === 0) {
    values.push(105) // Start C: two digits per symbol
    for (let i = 0; i < text.length; i += 2) values.push(Number(text.slice(i, i + 2)))
  } else {
    values.push(104) // Start B: ASCII 32..127
    for (const ch of text) {
      const c = ch.charCodeAt(0)
      if (c < 32 || c > 127) throw new Error('unsupported character')
      values.push(c - 32)
    }
  }
  const check = values.reduce((sum, v, i) => sum + v * (i === 0 ? 1 : i), 0) % 103
  return [...values, check, 106].map((v) => widthsToModules(C128[v]!)).join('')
}

export type Symbology = 'ean13' | 'ean8' | 'upca' | 'code128'
export interface BarcodePattern {
  symbology: Symbology
  modules: string
  /** Human-readable text under the bars. */
  text: string
}

/** Pick the right symbology for a product barcode/SKU and build its bar pattern (null if impossible). */
export function barcodePattern(raw: string): BarcodePattern | null {
  const code = (raw || '').trim()
  if (!code) return null
  try {
    if (/^\d{13}$/.test(code) && gtinValid(code)) return { symbology: 'ean13', modules: ean13(code), text: code }
    if (/^\d{12}$/.test(code) && gtinValid(code)) return { symbology: 'upca', modules: ean13('0' + code), text: code }
    if (/^\d{8}$/.test(code) && gtinValid(code)) return { symbology: 'ean8', modules: ean8(code), text: code }
    if (code.length > 40) return null
    return { symbology: 'code128', modules: code128(code), text: code }
  } catch {
    return null
  }
}
