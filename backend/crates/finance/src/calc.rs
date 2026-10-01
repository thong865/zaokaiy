//! Tax arithmetic (integer minor units, half-up rounding).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Taxes {
    pub gross_cents: i64,
    pub vat_cents: i64,
    pub net_cents: i64,
    pub ecommerce_tax_cents: i64,
    pub total_due_cents: i64,
}

fn div_round(n: i128, d: i128) -> i64 {
    ((n * 2 + d) / (d * 2)) as i64
}

/// `gross` is what customers paid. For a VAT-registered shop it includes VAT
/// (VAT = gross × rate / (1 + rate)); otherwise no VAT is due. E-commerce tax applies to the net.
pub fn compute(gross_cents: i64, vat_registered: bool, vat_bps: i32, ecommerce_tax_bps: i32) -> Taxes {
    let g = gross_cents.max(0) as i128;
    let vat = if vat_registered && vat_bps > 0 { div_round(g * vat_bps as i128, 10_000 + vat_bps as i128) } else { 0 };
    let net = gross_cents.max(0) - vat;
    let etax = div_round(net as i128 * ecommerce_tax_bps.max(0) as i128, 10_000);
    Taxes { gross_cents: gross_cents.max(0), vat_cents: vat, net_cents: net, ecommerce_tax_cents: etax, total_due_cents: vat + etax }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vat_inclusive_10_percent() {
        // 110.00 incl. 10% VAT → 10.00 VAT, 100.00 net; 1% e-commerce tax → 1.00
        let t = compute(11_000, true, 1000, 100);
        assert_eq!((t.vat_cents, t.net_cents, t.ecommerce_tax_cents, t.total_due_cents), (1_000, 10_000, 100, 1_100));
    }

    #[test]
    fn not_registered_pays_no_vat() {
        let t = compute(11_000, false, 1000, 100);
        assert_eq!((t.vat_cents, t.net_cents, t.ecommerce_tax_cents), (0, 11_000, 110));
    }

    #[test]
    fn rounds_half_up() {
        // 1.00 incl. 7% → 0.0654… → 7 cents
        assert_eq!(compute(100, true, 700, 0).vat_cents, 7);
        assert_eq!(compute(5, false, 0, 1000).ecommerce_tax_cents, 1); // 0.5 → 1
        assert_eq!(compute(0, true, 1000, 100).total_due_cents, 0);
    }

    #[test]
    fn big_amounts_do_not_overflow() {
        // 10 billion kip in att
        let t = compute(1_000_000_000_000, true, 1000, 100);
        assert_eq!(t.vat_cents + t.net_cents, 1_000_000_000_000);
    }
}
