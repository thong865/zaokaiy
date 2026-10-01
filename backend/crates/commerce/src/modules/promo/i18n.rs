//! Lao translations of this module's API error messages (merged by `crate::i18n`).

use std::sync::LazyLock;

use regex::Regex;

const EXACT: &[(&str, &str)] = &[
    ("another campaign already uses this code", "ມີແຄມເປນອື່ນໃຊ້ລະຫັດນີ້ແລ້ວ"),
    ("amounts can't be negative", "ຈຳນວນເງິນຕ້ອງບໍ່ຕິດລົບ"),
    ("bonus points are given with first-order campaigns", "ຄະແນນໂບນັດໃຫ້ໄດ້ສະເພາະແຄມເປນສັ່ງຊື້ຄັ້ງທຳອິດ"),
    ("cart is empty", "ກະຕ່າສິນຄ້າຫວ່າງເປົ່າ"),
    ("check the redeem rules: minimum 1 point, up to 100% of a bill, expiry 0–3650 days", "ກວດສອບເງື່ອນໄຂການໃຊ້ຄະແນນ: ຢ່າງໜ້ອຍ 1 ຄະແນນ, ບໍ່ເກີນ 100% ຂອງບິນ, ໝົດອາຍຸ 0–3650 ວັນ"),
    ("choose where it can be used: web, social or pos", "ເລືອກບ່ອນທີ່ໃຊ້ໄດ້: ເວັບ, ແຊັດ/ໂຊຊຽວ ຫຼື ໜ້າຮ້ານ (POS)"),
    ("coupon code not found", "ບໍ່ພົບລະຫັດຄູປອງ"),
    ("coupons must be valid for 1 to 3650 days", "ຄູປອງຕ້ອງມີອາຍຸ 1 ຫາ 3650 ວັນ"),
    ("currency must be LAK, THB or USD", "ສະກຸນເງິນຕ້ອງເປັນ LAK, THB ຫຼື USD"),
    ("earning and point value must be above zero", "ອັດຕາສະສົມ ແລະ ມູນຄ່າຄະແນນຕ້ອງຫຼາຍກວ່າສູນ"),
    ("enter a valid phone number", "ກະລຸນາໃສ່ເບີໂທທີ່ຖືກຕ້ອງ"),
    ("enter the points to add or remove and a note", "ກະລຸນາໃສ່ຄະແນນທີ່ຈະເພີ່ມ ຫຼື ຫັກ ພ້ອມໝາຍເຫດ"),
    ("enter the reward value", "ກະລຸນາໃສ່ມູນຄ່າລາງວັນ"),
    ("enter your phone number to use a code", "ກະລຸນາໃສ່ເບີໂທຂອງທ່ານເພື່ອໃຊ້ລະຫັດ"),
    ("give the campaign a name (up to 120 characters)", "ກະລຸນາຕັ້ງຊື່ແຄມເປນ (ບໍ່ເກີນ 120 ຕົວອັກສອນ)"),
    ("no points yet for this phone number", "ເບີໂທນີ້ຍັງບໍ່ມີຄະແນນ"),
    ("not enough points for this order", "ຄະແນນບໍ່ພໍສຳລັບຄຳສັ່ງຊື້ນີ້"),
    ("percent must be between 0.01% and 100%", "ເປີເຊັນຕ້ອງຢູ່ລະຫວ່າງ 0,01% ຫາ 100%"),
    ("sign in with your member phone number to use points", "ກະລຸນາເຂົ້າສູ່ລະບົບດ້ວຍເບີໂທສະມາຊິກເພື່ອໃຊ້ຄະແນນ"),
    ("sign-up methods must be google, facebook, whatsapp or email", "ວິທີສະໝັກຕ້ອງເປັນ Google, Facebook, WhatsApp ຫຼື ອີເມວ"),
    ("the balance can't go below zero", "ຍອດຄະແນນຕ້ອງບໍ່ຕ່ຳກວ່າສູນ"),
    ("the code must be 3–20 letters or digits", "ລະຫັດຕ້ອງເປັນຕົວອັກສອນ ຫຼື ຕົວເລກ 3–20 ຕົວ"),
    ("the end date must be after the start", "ວັນສິ້ນສຸດຕ້ອງຢູ່ຫຼັງວັນເລີ່ມ"),
    ("the order is below this coupon's minimum spend", "ຍອດສັ່ງຊື້ຍັງບໍ່ເຖິງຍອດຂັ້ນຕ່ຳຂອງຄູປອງນີ້"),
    ("this coupon belongs to another customer", "ຄູປອງນີ້ເປັນຂອງລູກຄ້າຄົນອື່ນ"),
    ("this coupon can't be used here", "ຄູປອງນີ້ໃຊ້ຢູ່ບ່ອນນີ້ບໍ່ໄດ້"),
    (
        "this coupon gives free shipping — choose a delivery option with shipping paid at checkout",
        "ຄູປອງນີ້ແມ່ນສົ່ງຟຣີ — ກະລຸນາເລືອກການຈັດສົ່ງທີ່ຈ່າຍຄ່າສົ່ງຕອນສັ່ງຊື້",
    ),
    ("this coupon has already been used", "ຄູປອງນີ້ຖືກໃຊ້ແລ້ວ"),
    ("this coupon has expired", "ຄູປອງນີ້ໝົດອາຍຸແລ້ວ"),
    ("this coupon is for another currency", "ຄູປອງນີ້ໃຊ້ກັບສະກຸນເງິນອື່ນ"),
    ("this coupon is for another shop", "ຄູປອງນີ້ໃຊ້ກັບຮ້ານອື່ນ"),
    ("this promotion has ended", "ໂປຣໂມຊັນນີ້ສິ້ນສຸດແລ້ວ"),
    ("this shop doesn't take loyalty points here", "ຮ້ານນີ້ບໍ່ຮັບຄະແນນສະສົມຢູ່ບ່ອນນີ້"),
    ("you have already used this code", "ທ່ານໃຊ້ລະຫັດນີ້ແລ້ວ"),
    ("trigger must be signup or code", "ເງື່ອນໄຂຕ້ອງເປັນ ສະໝັກສະມາຊິກ ຫຼື ລະຫັດ"),
    ("trigger must be first_order or code", "ເງື່ອນໄຂຕ້ອງເປັນ ສັ່ງຊື້ຄັ້ງທຳອິດ ຫຼື ລະຫັດ"),
    ("reward must be amount, percent or free_shipping", "ລາງວັນຕ້ອງເປັນ ສ່ວນຫຼຸດເປັນເງິນ, ເປີເຊັນ ຫຼື ສົ່ງຟຣີ"),
    ("reward must be amount, percent, free_shipping or points", "ລາງວັນຕ້ອງເປັນ ສ່ວນຫຼຸດເປັນເງິນ, ເປີເຊັນ, ສົ່ງຟຣີ ຫຼື ຄະແນນ"),
];

static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> =
    LazyLock::new(|| [(r"^use at least (\d+) points$", "ຕ້ອງໃຊ້ຢ່າງໜ້ອຍ $1 ຄະແນນ")].into_iter().map(|(p, t)| (Regex::new(p).unwrap(), t)).collect());

pub fn translate_lo(msg: &str) -> Option<String> {
    if let Some((_, lo)) = EXACT.iter().find(|(en, _)| *en == msg) {
        return Some((*lo).to_string());
    }
    PATTERNS.iter().find(|(re, _)| re.is_match(msg)).map(|(re, t)| re.replace(msg, *t).into_owned())
}
