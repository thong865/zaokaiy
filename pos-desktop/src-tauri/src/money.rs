//! Sale arithmetic — must match the server exactly (`backend/src/routes/pos.rs`), because the
//! server re-computes every pushed sale and rejects one whose totals differ.

use serde::{Deserialize, Serialize};

pub const METHODS: [&str; 5] = ["cash", "card", "transfer", "qr", "other"];
pub const MAX_LINES: usize = 200;

/// Round-half-up integer division.
fn div_round(a: i64, b: i64) -> i64 {
    (a * 2 + b) / (b * 2)
}

/// (vat, total) for a net amount after discounts.
pub fn vat_for(net: i64, bps: i32, inclusive: bool) -> (i64, i64) {
    let bps = bps as i64;
    if bps == 0 {
        return (0, net);
    }
    if inclusive {
        (div_round(net * bps, 10_000 + bps), net)
    } else {
        let vat = div_round(net * bps, 10_000);
        (vat, net + vat)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub product_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub sku: String,
    pub qty: i32,
    pub unit_price_cents: i64,
    #[serde(default)]
    pub discount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Payment {
    pub method: String,
    pub amount_cents: i64,
    #[serde(default)]
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Totals {
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub vat_cents: i64,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub change_cents: i64,
}

/// Validate a cart + payments and compute its totals (the same rules as the web till).
pub fn compute(lines: &[Line], bill_discount: i64, payments: &[Payment], vat_bps: i32, inclusive: bool) -> Result<Totals, String> {
    if lines.is_empty() {
        return Err("add at least one item".into());
    }
    if lines.len() > MAX_LINES {
        return Err(format!("at most {MAX_LINES} lines per sale"));
    }
    let mut subtotal = 0i64;
    for l in lines {
        if !(1..=10_000).contains(&l.qty) {
            return Err("qty must be between 1 and 10000".into());
        }
        if l.name.trim().is_empty() {
            return Err("custom items need a name".into());
        }
        if l.unit_price_cents < 0 {
            return Err("price cannot be negative".into());
        }
        let gross = l.unit_price_cents.checked_mul(l.qty as i64).ok_or("amount too large")?;
        if l.discount_cents < 0 || l.discount_cents > gross {
            return Err(format!("discount on '{}' is larger than the line", l.name.trim()));
        }
        subtotal += gross - l.discount_cents;
    }
    if bill_discount < 0 || bill_discount > subtotal {
        return Err("bill discount is larger than the subtotal".into());
    }
    let (vat, total) = vat_for(subtotal - bill_discount, vat_bps, inclusive);
    if payments.is_empty() && total > 0 {
        return Err("add a payment".into());
    }
    let (mut paid, mut cash) = (0i64, 0i64);
    for p in payments {
        if !METHODS.contains(&p.method.as_str()) {
            return Err("unknown payment method".into());
        }
        if p.amount_cents <= 0 {
            return Err("payment amounts must be positive".into());
        }
        paid += p.amount_cents;
        if p.method == "cash" {
            cash += p.amount_cents;
        }
    }
    if paid < total {
        return Err(format!("payment is short by {}", total - paid));
    }
    let change = paid - total;
    if change > cash {
        return Err("card/transfer/QR payments cannot exceed the amount due".into());
    }
    Ok(Totals { subtotal_cents: subtotal, discount_cents: bill_discount, vat_cents: vat, total_cents: total, paid_cents: paid, change_cents: change })
}

/// Barcode key used for lookups: scanner noise removed, and numeric GTINs without leading zeros so
/// UPC-A, EAN-13 and GTIN-14 forms of the same code match (like the server's `/pos/scan`).
pub fn code_key(raw: &str) -> String {
    let mut s = raw.trim().trim_matches(|c: char| c == '\r' || c == '\n' || c == '\t');
    // AIM symbology identifier, e.g. "]E0"
    if s.starts_with(']') && s.len() > 3 {
        s = &s[3..];
    }
    let s = s.trim();
    if (8..=14).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()) {
        let t = s.trim_start_matches('0');
        return if t.is_empty() { "0".into() } else { t.to_string() };
    }
    s.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(qty: i32, unit: i64, disc: i64) -> Line {
        Line { product_id: None, name: "x".into(), sku: String::new(), qty, unit_price_cents: unit, discount_cents: disc }
    }
    fn cash(a: i64) -> Payment {
        Payment { method: "cash".into(), amount_cents: a, reference: String::new() }
    }

    #[test]
    fn vat_matches_server() {
        assert_eq!(vat_for(10_700, 700, true), (700, 10_700));
        assert_eq!(vat_for(10_000, 700, false), (700, 10_700));
        assert_eq!(vat_for(8_000, 700, true), (523, 8_000));
        assert_eq!(vat_for(999, 0, true), (0, 999));
    }

    #[test]
    fn totals_and_change() {
        let t = compute(&[l(2, 2500, 0), l(1, 4000, 500)], 500, &[cash(10_000)], 700, true).unwrap();
        assert_eq!((t.subtotal_cents, t.vat_cents, t.total_cents, t.change_cents), (8500, 523, 8000, 2000));
        let card = Payment { method: "card".into(), amount_cents: 9000, reference: String::new() };
        assert!(compute(&[l(1, 8000, 0)], 0, &[card], 0, true).is_err()); // change only from cash
        assert!(compute(&[l(1, 8000, 0)], 0, &[cash(7000)], 0, true).unwrap_err().starts_with("payment is short"));
        assert!(compute(&[], 0, &[], 0, true).is_err());
    }

    #[test]
    fn codes() {
        assert_eq!(code_key("0012345678905"), code_key("012345678905"));
        assert_eq!(code_key("]E00885909950805\r\n"), "885909950805");
        assert_eq!(code_key(" SKU-01 "), "sku-01");
    }
}
