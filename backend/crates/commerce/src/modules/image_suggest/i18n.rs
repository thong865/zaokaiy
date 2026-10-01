//! Lao translations of this module's API error messages (merged by `crate::i18n`).

const EXACT: &[(&str, &str)] = &[
    ("source must be stock, own or shared", "ແຫຼ່ງທີ່ມາຕ້ອງເປັນ ຄັງຮູບຂອງລະບົບ, ຄັງສື່ຂອງທ່ານ ຫຼື ຮູບທີ່ຮ້ານອື່ນແບ່ງປັນ"),
    ("stock photos must be images (JPG, PNG, WebP)", "ຮູບໃນຄັງຮູບຕ້ອງເປັນໄຟລ໌ຮູບພາບ (JPG, PNG, WebP)"),
    ("give the photo a title (at least 2 characters)", "ກະລຸນາໃສ່ຊື່ຮູບ (ຢ່າງໜ້ອຍ 2 ຕົວອັກສອນ)"),
    ("enter 2 to 20 words that mean the same thing", "ກະລຸນາໃສ່ຄຳທີ່ມີຄວາມໝາຍດຽວກັນ 2 ຫາ 20 ຄຳ"),
];

pub fn translate_lo(msg: &str) -> Option<String> {
    EXACT.iter().find(|(en, _)| *en == msg).map(|(_, lo)| (*lo).to_string())
}
