//! Lao translations of this module's API error messages (merged by `crate::i18n`).

use std::sync::LazyLock;

use regex::Regex;

const EXACT: &[(&str, &str)] = &[
    ("reason must be refused, unreachable, fake_address, no_show, changed_mind or other", "ເຫດຜົນຕ້ອງເປັນ ປະຕິເສດຮັບເຄື່ອງ, ຕິດຕໍ່ບໍ່ໄດ້, ທີ່ຢູ່ປອມ, ບໍ່ມາຮັບເຄື່ອງ, ປ່ຽນໃຈ ຫຼື ອື່ນໆ"),
    ("describe what happened", "ກະລຸນາອະທິບາຍສິ່ງທີ່ເກີດຂຶ້ນ"),
    ("only refused (returned) COD parcels can be reported", "ລາຍງານໄດ້ສະເພາະພັດສະດຸເກັບເງິນປາຍທາງທີ່ຖືກຕີກັບເທົ່ານັ້ນ"),
    ("this parcel came back too long ago to report", "ພັດສະດຸນີ້ຕີກັບມາດົນເກີນໄປ ບໍ່ສາມາດລາຍງານໄດ້ແລ້ວ"),
    ("this order has no valid customer phone number", "ຄຳສັ່ງຊື້ນີ້ບໍ່ມີເບີໂທລູກຄ້າທີ່ຖືກຕ້ອງ"),
    ("this order has already been reported", "ຄຳສັ່ງຊື້ນີ້ຖືກລາຍງານແລ້ວ"),
    ("only reports waiting for review can be withdrawn", "ຖອນໄດ້ສະເພາະລາຍງານທີ່ກຳລັງລໍຖ້າການກວດເທົ່ານັ້ນ"),
    ("enter between 1 and 50 phone numbers", "ກະລຸນາໃສ່ເບີໂທ 1 ຫາ 50 ເບີ"),
    ("action must be confirm or dismiss", "ການດຳເນີນການຕ້ອງເປັນ ຢືນຢັນ ຫຼື ຍົກເລີກລາຍງານ"),
    ("add a note explaining the decision", "ກະລຸນາຂຽນໝາຍເຫດອະທິບາຍການຕັດສິນໃຈ"),
    ("level must be none, low, medium, high or blocked", "ລະດັບຕ້ອງເປັນ ບໍ່ມີ, ຕ່ຳ, ປານກາງ, ສູງ ຫຼື ບລັອກ"),
    ("expiry must be between 1 and 3650 days", "ໄລຍະໝົດອາຍຸຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 3650 ວັນ"),
    ("add a note explaining the risk level", "ກະລຸນາຂຽນໝາຍເຫດອະທິບາຍລະດັບຄວາມສ່ຽງ"),
    ("cash on delivery is not available for this phone number — please pay online", "ເບີໂທນີ້ບໍ່ສາມາດໃຊ້ການເກັບເງິນປາຍທາງໄດ້ — ກະລຸນາຊຳລະເງິນອອນລາຍ"),
    (
        "anchoring is not configured (set COD_RISK_ANCHOR=webhook and COD_RISK_ANCHOR_URL)",
        "ຍັງບໍ່ໄດ້ຕັ້ງຄ່າການບັນທຶກລົງບລັອກເຊນ (ຕັ້ງ COD_RISK_ANCHOR=webhook ແລະ COD_RISK_ANCHOR_URL)",
    ),
];

static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [(r"^cannot (\w+) a report that is (\w+)$", "ບໍ່ສາມາດ $1 ລາຍງານທີ່ຢູ່ໃນສະຖານະ $2 ໄດ້")]
        .into_iter()
        .map(|(re, tpl)| (Regex::new(re).expect("valid cod_risk i18n regex"), tpl))
        .collect()
});

pub fn translate_lo(msg: &str) -> Option<String> {
    if let Some((_, lo)) = EXACT.iter().find(|(en, _)| *en == msg) {
        return Some((*lo).to_string());
    }
    PATTERNS.iter().find(|(re, _)| re.is_match(msg)).map(|(re, tpl)| re.replace(msg, *tpl).into_owned())
}
