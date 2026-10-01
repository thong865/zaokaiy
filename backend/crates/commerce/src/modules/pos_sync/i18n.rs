//! Lao translations of this module's error messages (merged by `crate::i18n`).

use std::sync::LazyLock;

use regex::Regex;

const EXACT: &[(&str, &str)] = &[
    ("give this terminal a name", "ກະລຸນາຕັ້ງຊື່ເຄື່ອງຂາຍນີ້"),
    ("invalid terminal id", "ລະຫັດເຄື່ອງຂາຍບໍ່ຖືກຕ້ອງ"),
    ("this pairing code is invalid or has expired", "ລະຫັດເຊື່ອມຕໍ່ບໍ່ຖືກຕ້ອງ ຫຼື ໝົດອາຍຸແລ້ວ"),
    ("this pairing code has already been used", "ລະຫັດເຊື່ອມຕໍ່ນີ້ຖືກໃຊ້ແລ້ວ"),
    ("invalid sync cursor", "ຕົວຊີ້ການຊິງຂໍ້ມູນບໍ່ຖືກຕ້ອງ"),
    ("sale id already used", "ລະຫັດການຂາຍນີ້ຖືກໃຊ້ແລ້ວ"),
    ("receipt number already used", "ເລກໃບບິນນີ້ຖືກໃຊ້ແລ້ວ"),
    ("send at most 100 sales and 100 voids at a time", "ສົ່ງໄດ້ເທື່ອລະບໍ່ເກີນ 100 ການຂາຍ ແລະ 100 ການຍົກເລີກ"),
    ("invalid receipt number", "ເລກໃບບິນບໍ່ຖືກຕ້ອງ"),
    ("invalid VAT rate", "ອັດຕາອາກອນບໍ່ຖືກຕ້ອງ"),
    ("every line needs a name", "ທຸກລາຍການຕ້ອງມີຊື່"),
    ("amount too large", "ຈຳນວນເງິນໃຫຍ່ເກີນໄປ"),
    ("invalid payment", "ການຊຳລະເງິນບໍ່ຖືກຕ້ອງ"),
    ("payments do not add up", "ຍອດການຊຳລະເງິນບໍ່ຖືກຕ້ອງ"),
];

static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"^a sale needs 1 to (\d+) lines$", "ການຂາຍຕ້ອງມີ 1 ຫາ $1 ລາຍການ"),
        (r"^totals do not match \(server: subtotal (-?\d+), VAT (-?\d+), total (-?\d+)\)$", "ຍອດລວມບໍ່ກົງກັນ (ເຊີບເວີ: ຍອດຍ່ອຍ $1, ອາກອນ $2, ລວມ $3)"),
    ]
    .into_iter()
    .map(|(re, tpl)| (Regex::new(re).expect("valid pos_sync i18n regex"), tpl))
    .collect()
});

pub fn translate_lo(msg: &str) -> Option<String> {
    if let Some((_, lo)) = EXACT.iter().find(|(en, _)| *en == msg) {
        return Some((*lo).to_string());
    }
    PATTERNS.iter().find(|(re, _)| re.is_match(msg)).map(|(re, tpl)| re.replace(msg, *tpl).into_owned())
}
