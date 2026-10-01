//! Lao translations for API error messages.
//!
//! Handlers keep producing English messages; [`localize_errors`] rewrites the `message` field of
//! error responses (status >= 400) when the request's `Accept-Language` primary tag is `lo`.
//! Success responses are never buffered or touched (media streaming stays zero-copy).

use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use regex::Regex;
use serde_json::{json, Value};

/// Error bodies are tiny; anything larger than this is passed through untouched.
const MAX_ERROR_BODY: usize = 64 * 1024;

/// Exact English message → Lao.
static EXACT: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        // generic defaults (AppError variants without a message)
        ("not found", "ບໍ່ພົບຂໍ້ມູນ"),
        ("unauthorized", "ກະລຸນາເຂົ້າສູ່ລະບົບ"),
        ("forbidden", "ທ່ານບໍ່ມີສິດເຂົ້າເຖິງ"),
        ("internal server error", "ເກີດຂໍ້ຜິດພາດໃນລະບົບ, ກະລຸນາລອງໃໝ່ພາຍຫຼັງ"),
        ("already exists", "ມີຢູ່ແລ້ວ"),
        ("service temporarily unavailable", "ບໍລິການບໍ່ພ້ອມໃຊ້ງານຊົ່ວຄາວ, ກະລຸນາລອງໃໝ່"),
        ("referenced record does not exist", "ບໍ່ພົບຂໍ້ມູນທີ່ອ້າງອີງ"),
        ("invalid JSON", "JSON ບໍ່ຖືກຕ້ອງ"),
        // corporate KYC (business verification)
        ("shop type must be individual or business", "ປະເພດຮ້ານຕ້ອງເປັນ ບຸກຄົນ ຫຼື ນິຕິບຸກຄົນ"),
        ("a verified or in-review business shop can't be changed to individual", "ຮ້ານນິຕິບຸກຄົນທີ່ຢືນຢັນແລ້ວ ຫຼື ກຳລັງກວດ ບໍ່ສາມາດປ່ຽນເປັນບຸກຄົນໄດ້"),
        ("ID and account numbers must be 4-40 letters or digits", "ເລກບັດ ແລະ ເລກບັນຊີຕ້ອງເປັນຕົວອັກສອນ ຫຼື ຕົວເລກ 4-40 ຕົວ"),
        ("a field in the verification form is too long", "ມີຂໍ້ມູນໃນແບບຟອມຢືນຢັນທີ່ຍາວເກີນໄປ"),
        ("accepted documents can't be deleted — upload a newer one instead", "ບໍ່ສາມາດລຶບເອກະສານທີ່ອະນຸມັດແລ້ວ — ກະລຸນາອັບໂຫຼດສະບັບໃໝ່ແທນ"),
        ("action must be approve, request_changes, reject or revoke", "ການດຳເນີນການຕ້ອງເປັນ ອະນຸມັດ, ຂໍໃຫ້ແກ້ໄຂ, ປະຕິເສດ ຫຼື ຖອນການຢືນຢັນ"),
        ("add a note explaining the decision to the seller", "ກະລຸນາຂຽນໝາຍເຫດອະທິບາຍການຕັດສິນໃຈໃຫ້ຜູ້ຂາຍ"),
        ("at most 20 people", "ໃສ່ໄດ້ບໍ່ເກີນ 20 ຄົນ"),
        ("at most 30 documents — delete old ones first", "ເອກະສານໄດ້ບໍ່ເກີນ 30 ລາຍການ — ກະລຸນາລຶບເອກະສານເກົ່າກ່ອນ"),
        ("country must be a 2-letter code, e.g. LA or TH", "ປະເທດຕ້ອງເປັນລະຫັດ 2 ຕົວອັກສອນ ເຊັ່ນ LA ຫຼື TH"),
        ("document status must be accepted, rejected or pending", "ສະຖານະເອກະສານຕ້ອງເປັນ ອະນຸມັດ, ປະຕິເສດ ຫຼື ລໍຖ້າ"),
        ("documents can be at most 10 MB", "ເອກະສານຕ້ອງມີຂະໜາດບໍ່ເກີນ 10 MB"),
        ("documents must be JPG, PNG, WebP or PDF", "ເອກະສານຕ້ອງເປັນໄຟລ໌ JPG, PNG, WebP ຫຼື PDF"),
        ("each person needs a full name", "ກະລຸນາໃສ່ຊື່ ແລະ ນາມສະກຸນຂອງແຕ່ລະຄົນ"),
        ("each person needs a role: representative, director or beneficial owner", "ແຕ່ລະຄົນຕ້ອງມີບົດບາດ: ຜູ້ຕາງໜ້າ, ຜູ້ອຳນວຍການ ຫຼື ເຈົ້າຂອງຜູ້ໄດ້ຮັບຜົນປະໂຫຍດ"),
        ("enter a valid date of birth", "ກະລຸນາໃສ່ວັນເດືອນປີເກີດໃຫ້ຖືກຕ້ອງ"),
        ("expiry date must be YYYY-MM-DD", "ວັນໝົດອາຍຸຕ້ອງເປັນຮູບແບບ YYYY-MM-DD"),
        ("only submitted verifications can be decided", "ຕັດສິນໄດ້ສະເພາະຄຳຮ້ອງທີ່ສົ່ງມາແລ້ວເທົ່ານັ້ນ"),
        ("ownership must be between 0 and 100%", "ສັດສ່ວນຮຸ້ນຕ້ອງຢູ່ລະຫວ່າງ 0 ຫາ 100%"),
        ("registration date cannot be in the future", "ວັນທີຈົດທະບຽນຕ້ອງບໍ່ເປັນວັນໃນອະນາຄົດ"),
        ("required documents are rejected or missing — request changes instead", "ເອກະສານທີ່ຈຳເປັນຖືກປະຕິເສດ ຫຼື ຍັງບໍ່ຄົບ — ກະລຸນາຂໍໃຫ້ແກ້ໄຂແທນ"),
        ("say why the document is rejected", "ກະລຸນາບອກເຫດຜົນທີ່ປະຕິເສດເອກະສານ"),
        ("set the shop type to business first", "ກະລຸນາຕັ້ງປະເພດຮ້ານເປັນນິຕິບຸກຄົນກ່ອນ"),
        ("some required information or documents are missing", "ຂໍ້ມູນ ຫຼື ເອກະສານທີ່ຈຳເປັນຍັງບໍ່ຄົບ"),
        ("this document has already expired", "ເອກະສານນີ້ໝົດອາຍຸແລ້ວ"),
        ("this shop is not verified", "ຮ້ານນີ້ຍັງບໍ່ໄດ້ຮັບການຢືນຢັນ"),
        ("unknown ID document type", "ບໍ່ຮູ້ຈັກປະເພດເອກະສານຢັ້ງຢືນຕົວຕົນນີ້"),
        ("unknown company type", "ບໍ່ຮູ້ຈັກປະເພດວິສາຫະກິດນີ້"),
        ("unknown document type", "ບໍ່ຮູ້ຈັກປະເພດເອກະສານນີ້"),
        ("upload interrupted or too large", "ການອັບໂຫຼດຂາດຕອນ ຫຼື ໄຟລ໌ໃຫຍ່ເກີນໄປ"),
        ("verify your business before publishing products", "ກະລຸນາຢືນຢັນທຸລະກິດຂອງທ່ານກ່ອນເຜີຍແຜ່ສິນຄ້າ"),
        ("website must start with https://", "ເວັບໄຊຕ້ອງເລີ່ມດ້ວຍ https://"),
        ("your verification is already under review", "ຄຳຮ້ອງຢືນຢັນຂອງທ່ານກຳລັງຢູ່ໃນການກວດແລ້ວ"),
        ("your verification is under review — wait for the decision before editing", "ຄຳຮ້ອງຢືນຢັນຂອງທ່ານກຳລັງຢູ່ໃນການກວດ — ກະລຸນາລໍຖ້າຜົນກ່ອນແກ້ໄຂ"),
        // vehicle sellers
        ("this store type is not available", "ປະເພດຮ້ານນີ້ບໍ່ເປີດໃຫ້ໃຊ້"),
        ("make and model are required", "ກະລຸນາໃສ່ຍີ່ຫໍ້ ແລະ ລຸ້ນລົດ"),
        ("make, model or variant is too long", "ຍີ່ຫໍ້, ລຸ້ນ ຫຼື ລຸ້ນຍ່ອຍຍາວເກີນໄປ"),
        ("enter a valid model year", "ກະລຸນາໃສ່ປີລົດໃຫ້ຖືກຕ້ອງ"),
        ("mileage must be between 0 and 5,000,000 km", "ເລກໄມຕ້ອງຢູ່ລະຫວ່າງ 0 ຫາ 5,000,000 ກມ"),
        ("vehicle type must be car, motorbike, truck, van or other", "ປະເພດລົດຕ້ອງເປັນ ລົດໃຫຍ່, ລົດຈັກ, ລົດບັນທຸກ, ລົດຕູ້ ຫຼື ອື່ນໆ"),
        ("condition must be new, used or reconditioned", "ສະພາບຕ້ອງເປັນ ລົດໃໝ່, ລົດມືສອງ ຫຼື ລົດປັບສະພາບ"),
        ("unknown fuel type", "ບໍ່ຮູ້ຈັກປະເພດນ້ຳມັນເຊື້ອໄຟນີ້"),
        ("unknown transmission", "ບໍ່ຮູ້ຈັກປະເພດເກຍນີ້"),
        ("unknown drive type", "ບໍ່ຮູ້ຈັກລະບົບຂັບເຄື່ອນນີ້"),
        ("unknown body type", "ບໍ່ຮູ້ຈັກຮູບແບບຕົວຖັງນີ້"),
        ("sale status must be available, reserved or sold", "ສະຖານະການຂາຍຕ້ອງເປັນ ພ້ອມຂາຍ, ຈອງແລ້ວ ຫຼື ຂາຍແລ້ວ"),
        ("a number in the vehicle specs is out of range", "ມີຕົວເລກໃນຂໍ້ມູນລົດທີ່ເກີນຂອບເຂດ"),
        ("deposit cannot be negative", "ເງິນມັດຈຳຕ້ອງບໍ່ຕິດລົບ"),
        ("VIN / chassis number must be 6-20 letters or digits", "ເລກຖັງ (VIN) ຕ້ອງເປັນຕົວອັກສອນ ຫຼື ຕົວເລກ 6-20 ຕົວ"),
        ("a text field in the vehicle specs is too long", "ມີຂໍ້ຄວາມໃນຂໍ້ມູນລົດທີ່ຍາວເກີນໄປ"),
        ("at most 40 features", "ໃສ່ອຸປະກອນເສີມໄດ້ບໍ່ເກີນ 40 ລາຍການ"),
        ("this product has no vehicle details", "ສິນຄ້ານີ້ບໍ່ມີຂໍ້ມູນລົດ"),
        ("request type must be enquiry, test_drive, offer, reserve or finance", "ປະເພດຄຳຂໍຕ້ອງເປັນ ສອບຖາມ, ທົດລອງຂັບ, ຕໍ່ລາຄາ, ຈອງ ຫຼື ສິນເຊື່ອ"),
        ("enter your name", "ກະລຸນາໃສ່ຊື່ຂອງທ່ານ"),
        ("enter a valid phone number", "ກະລຸນາໃສ່ເບີໂທໃຫ້ຖືກຕ້ອງ"),
        ("message is too long", "ຂໍ້ຄວາມຍາວເກີນໄປ"),
        ("this vehicle has already been sold", "ລົດຄັນນີ້ຂາຍແລ້ວ"),
        ("choose a date and time for the test drive", "ກະລຸນາເລືອກວັນ ແລະ ເວລາທົດລອງຂັບ"),
        ("the test drive time must be in the future", "ເວລາທົດລອງຂັບຕ້ອງເປັນເວລາໃນອະນາຄົດ"),
        ("choose a test drive time within the next 90 days", "ກະລຸນາເລືອກເວລາທົດລອງຂັບພາຍໃນ 90 ວັນ"),
        ("your offer is higher than twice the asking price", "ລາຄາທີ່ທ່ານສະເໜີສູງກວ່າສອງເທົ່າຂອງລາຄາຂາຍ"),
        ("enter the amount you want to offer", "ກະລຸນາໃສ່ລາຄາທີ່ທ່ານຕ້ອງການສະເໜີ"),
        ("this vehicle is already reserved", "ລົດຄັນນີ້ມີຄົນຈອງແລ້ວ"),
        ("too many requests from this phone number, try again later", "ມີຄຳຂໍຈາກເບີໂທນີ້ຫຼາຍເກີນໄປ, ກະລຸນາລອງໃໝ່ພາຍຫຼັງ"),
        ("status must be new, contacted, scheduled, won or lost", "ສະຖານະຕ້ອງເປັນ ໃໝ່, ຕິດຕໍ່ແລ້ວ, ນັດໝາຍແລ້ວ, ປິດການຂາຍ ຫຼື ບໍ່ສຳເລັດ"),
        ("note is too long", "ບັນທຶກຍາວເກີນໄປ"),
        // auth / accounts
        ("email already registered", "ອີເມວນີ້ລົງທະບຽນແລ້ວ"),
        ("invalid email or password", "ອີເມວ ຫຼື ລະຫັດຜ່ານບໍ່ຖືກຕ້ອງ"),
        ("invalid email", "ອີເມວບໍ່ຖືກຕ້ອງ"),
        ("password must be at least 8 characters", "ລະຫັດຜ່ານຕ້ອງມີຢ່າງໜ້ອຍ 8 ຕົວອັກສອນ"),
        ("display_name is required", "ກະລຸນາໃສ່ຊື່ສະແດງ (display_name)"),
        ("kind must be seller or creator", "ປະເພດຕ້ອງເປັນຜູ້ຂາຍ ຫຼື ຄຣີເອເຕີ"),
        ("currency must be THB, LAK or USD", "ສະກຸນເງິນຕ້ອງເປັນ THB, LAK ຫຼື USD"),
        ("unknown courier", "ບໍ່ພົບບໍລິສັດຂົນສົ່ງນີ້"),
        ("empty barcode", "ບາໂຄດວ່າງເປົ່າ"),
        ("select at most 1000 products", "ເລືອກສິນຄ້າໄດ້ບໍ່ເກີນ 1000 ລາຍການ"),
        ("ship the order before recording the cash", "ກະລຸນາສົ່ງສິນຄ້າກ່ອນ ຈຶ່ງບັນທຶກການເກັບເງິນໄດ້"),
        ("choose a delivery option", "ກະລຸນາເລືອກວິທີຈັດສົ່ງ"),
        ("cash on delivery is not available with this courier", "ບໍລິສັດຂົນສົ່ງນີ້ບໍ່ຮອງຮັບການເກັບເງິນປາຍທາງ (COD)"),
        ("choose cash on delivery with your delivery details", "ກະລຸນາເລືອກເກັບເງິນປາຍທາງ (COD) ພ້ອມກັບຂໍ້ມູນການຈັດສົ່ງ"),
        ("links must start with https://", "ລິ້ງຕ້ອງເລີ່ມດ້ວຍ https://"),
        ("code must be 2-30 lowercase letters, digits or _", "ລະຫັດຕ້ອງເປັນຕົວອັກສອນພິມນ້ອຍ, ຕົວເລກ ຫຼື _ ຈຳນວນ 2-30 ຕົວ"),
        ("a courier with this code already exists", "ມີບໍລິສັດຂົນສົ່ງທີ່ໃຊ້ລະຫັດນີ້ແລ້ວ"),
        ("fee_payer must be buyer, destination or seller", "ຜູ້ຈ່າຍຄ່າສົ່ງຕ້ອງເປັນ ຜູ້ຊື້, ປາຍທາງ ຫຼື ຜູ້ຂາຍ"),
        ("fees cannot be negative", "ຄ່າທຳນຽມຕ້ອງບໍ່ຕິດລົບ"),
        ("this courier does not collect cash on delivery", "ບໍລິສັດຂົນສົ່ງນີ້ບໍ່ເກັບເງິນປາຍທາງ"),
        ("delivery must be branch pickup or home delivery", "ການຈັດສົ່ງຕ້ອງເປັນ ຮັບທີ່ສາຂາ ຫຼື ສົ່ງເຖິງບ້ານ"),
        ("this delivery option is not available", "ວິທີຈັດສົ່ງນີ້ບໍ່ພ້ອມໃຊ້ງານ"),
        ("home delivery is not offered with this courier", "ບໍລິສັດຂົນສົ່ງນີ້ບໍ່ມີບໍລິການສົ່ງເຖິງບ້ານ"),
        ("status must be collected, remitted or returned", "ສະຖານະຕ້ອງເປັນ ເກັບເງິນແລ້ວ, ໂອນໃຫ້ຮ້ານແລ້ວ ຫຼື ຕີກັບ"),
        ("select between 1 and 200 orders", "ກະລຸນາເລືອກ 1 ຫາ 200 ຄຳສັ່ງຊື້"),
        ("kind must be order or social", "ປະເພດຕ້ອງເປັນ order ຫຼື social"),
        ("payment method must be prepaid or cod", "ວິທີຊຳລະເງິນຕ້ອງເປັນ ຈ່າຍກ່ອນ ຫຼື ເກັບເງິນປາຍທາງ"),
        ("this order is paid in cash on delivery", "ຄຳສັ່ງຊື້ນີ້ຊຳລະແບບເກັບເງິນປາຍທາງ"),
        ("this login method is not enabled", "ວິທີເຂົ້າສູ່ລະບົບນີ້ຍັງບໍ່ໄດ້ເປີດໃຊ້"),
        ("this sign-in link has expired, please try again", "ລິ້ງເຂົ້າສູ່ລະບົບນີ້ໝົດອາຍຸແລ້ວ, ກະລຸນາລອງໃໝ່"),
        ("enter a valid phone number with country code", "ກະລຸນາໃສ່ເບີໂທທີ່ຖືກຕ້ອງພ້ອມລະຫັດປະເທດ"),
        ("too many codes requested, try again in an hour", "ຂໍລະຫັດຫຼາຍເກີນໄປ, ກະລຸນາລອງໃໝ່ໃນອີກໜຶ່ງຊົ່ວໂມງ"),
        ("the code has expired, request a new one", "ລະຫັດໝົດອາຍຸແລ້ວ, ກະລຸນາຂໍລະຫັດໃໝ່"),
        ("too many wrong attempts, request a new code", "ໃສ່ລະຫັດຜິດຫຼາຍເທື່ອເກີນໄປ, ກະລຸນາຂໍລະຫັດໃໝ່"),
        ("wrong code", "ລະຫັດບໍ່ຖືກຕ້ອງ"),
        // human check (Cloudflare Turnstile) + two-factor authentication
        ("please complete the human verification", "ກະລຸນາຢືນຢັນວ່າທ່ານເປັນມະນຸດ"),
        ("human verification failed, please try again", "ການຢືນຢັນວ່າເປັນມະນຸດບໍ່ສຳເລັດ, ກະລຸນາລອງໃໝ່"),
        ("could not check the human verification, please try again", "ບໍ່ສາມາດກວດສອບການຢືນຢັນວ່າເປັນມະນຸດໄດ້, ກະລຸນາລອງໃໝ່"),
        ("this sign-in attempt has expired, please sign in again", "ການເຂົ້າສູ່ລະບົບນີ້ໝົດອາຍຸແລ້ວ, ກະລຸນາເຂົ້າສູ່ລະບົບໃໝ່"),
        ("two-factor authentication is not turned on", "ຍັງບໍ່ໄດ້ເປີດການຢືນຢັນສອງຂັ້ນຕອນ"),
        ("two-factor authentication is already on", "ການຢືນຢັນສອງຂັ້ນຕອນເປີດຢູ່ແລ້ວ"),
        ("too many wrong codes, try again in 15 minutes", "ໃສ່ລະຫັດຜິດຫຼາຍເທື່ອເກີນໄປ, ກະລຸນາລອງໃໝ່ໃນອີກ 15 ນາທີ"),
        ("start the setup again", "ກະລຸນາເລີ່ມການຕັ້ງຄ່າໃໝ່"),
        ("this login is already connected to another account", "ວິທີເຂົ້າສູ່ລະບົບນີ້ເຊື່ອມກັບບັນຊີອື່ນແລ້ວ"),
        ("add a password or another login method before disconnecting this one", "ກະລຸນາຕັ້ງລະຫັດຜ່ານ ຫຼື ເຊື່ອມວິທີເຂົ້າສູ່ລະບົບອື່ນກ່ອນ ຈຶ່ງຈະຕັດການເຊື່ອມຕໍ່ອັນນີ້ໄດ້"),
        ("current password is incorrect", "ລະຫັດຜ່ານປັດຈຸບັນບໍ່ຖືກຕ້ອງ"),
        ("add an email or phone number first, so you can sign in with the password", "ກະລຸນາເພີ່ມອີເມວ ຫຼື ເບີໂທກ່ອນ ເພື່ອໃຫ້ເຂົ້າສູ່ລະບົບດ້ວຍລະຫັດຜ່ານໄດ້"),
        // shops
        ("shop slug already taken", "slug ຂອງຮ້ານນີ້ຖືກໃຊ້ແລ້ວ"),
        ("slug must be 3-40 chars: a-z, 0-9, '-'", "slug ຕ້ອງມີ 3-40 ຕົວ: a-z, 0-9, '-'"),
        ("could not find a free slug", "ບໍ່ພົບ slug ທີ່ຍັງວ່າງ"),
        ("name cannot be empty", "ຊື່ຕ້ອງບໍ່ຫວ່າງ"),
        ("name is required", "ກະລຸນາໃສ່ຊື່"),
        ("name is required (max 80 characters)", "ກະລຸນາໃສ່ຊື່ (ສູງສຸດ 80 ຕົວອັກສອນ)"),
        ("name is too long", "ຊື່ຍາວເກີນໄປ"),
        ("vat_bps must be between 0 and 3000 (0-30%)", "ອາກອນມູນຄ່າເພີ່ມ (vat_bps) ຕ້ອງຢູ່ລະຫວ່າງ 0 ຫາ 3000 (0-30%)"),
        ("prefixes may use up to 8 letters, digits or '-'", "ຄຳນຳໜ້າໃຊ້ຕົວອັກສອນ, ຕົວເລກ ຫຼື '-' ໄດ້ບໍ່ເກີນ 8 ຕົວ"),
        // products & inventory
        ("name and sku are required", "ກະລຸນາໃສ່ຊື່ ແລະ SKU"),
        ("price_cents must be >= 0", "ລາຄາ (price_cents) ຕ້ອງ >= 0"),
        ("price cannot be negative", "ລາຄາຕ້ອງບໍ່ຕິດລົບ"),
        ("initial_stock must be >= 0", "ສະຕັອກເລີ່ມຕົ້ນ (initial_stock) ຕ້ອງ >= 0"),
        ("status must be draft, active or archived", "ສະຖານະຕ້ອງເປັນ ສະບັບຮ່າງ (draft), ເປີດຂາຍ (active) ຫຼື ເກັບໄວ້ (archived)"),
        ("SKU, barcode or social code already exists in this shop", "SKU, ບາໂຄດ ຫຼື ລະຫັດໂຊຊຽວນີ້ ມີຢູ່ໃນຮ້ານແລ້ວ"),
        ("barcode or social code already used by another product", "ບາໂຄດ ຫຼື ລະຫັດໂຊຊຽວນີ້ ຖືກໃຊ້ກັບສິນຄ້າອື່ນແລ້ວ"),
        ("social code must be 1–10 letters/digits and contain a letter, e.g. A01", "ລະຫັດໂຊຊຽວຕ້ອງມີ 1–10 ຕົວອັກສອນ/ຕົວເລກ ແລະ ມີຕົວອັກສອນຢ່າງໜ້ອຍ 1 ຕົວ, ເຊັ່ນ A01"),
        ("product not found", "ບໍ່ພົບສິນຄ້າ"),
        ("product belongs to another shop", "ສິນຄ້ານີ້ເປັນຂອງຮ້ານອື່ນ"),
        ("delta must be non-zero", "ຈຳນວນທີ່ປ່ຽນ (delta) ຕ້ອງບໍ່ແມ່ນ 0"),
        ("reason must be restock, adjust or return", "ເຫດຜົນຕ້ອງເປັນ ເຕີມສະຕັອກ (restock), ປັບຈຳນວນ (adjust) ຫຼື ສົ່ງຄືນ (return)"),
        ("stock cannot go below zero", "ສະຕັອກຕ້ອງບໍ່ຕ່ຳກວ່າ 0"),
        // categories
        ("a category cannot be moved inside itself", "ບໍ່ສາມາດຍ້າຍໝວດໝູ່ເຂົ້າໄປໃນຕົວມັນເອງໄດ້"),
        ("a shop can have at most 200 categories", "ຮ້ານໜຶ່ງມີໝວດໝູ່ໄດ້ສູງສຸດ 200 ໝວດ"),
        ("all categories must share the same parent", "ທຸກໝວດໝູ່ຕ້ອງຢູ່ພາຍໃຕ້ໝວດໝູ່ຫຼັກດຽວກັນ"),
        ("parent category belongs to a different tree", "ໝວດໝູ່ຫຼັກຢູ່ໃນໂຄງສ້າງໝວດໝູ່ອື່ນ"),
        ("parent category not found", "ບໍ່ພົບໝວດໝູ່ຫຼັກ"),
        ("shop category not found in this shop", "ບໍ່ພົບໝວດໝູ່ນີ້ໃນຮ້ານ"),
        ("choose a marketplace category before publishing", "ກະລຸນາເລືອກໝວດໝູ່ຕະຫຼາດກ່ອນເຜີຍແຜ່"),
        ("reassign_to must be another marketplace category", "reassign_to ຕ້ອງເປັນໝວດໝູ່ຕະຫຼາດອື່ນ"),
        ("that marketplace category is no longer available", "ໝວດໝູ່ຕະຫຼາດນີ້ບໍ່ມີໃຫ້ໃຊ້ແລ້ວ"),
        ("unknown marketplace category", "ບໍ່ຮູ້ຈັກໝວດໝູ່ຕະຫຼາດນີ້"),
        // review
        ("action must be approve or reject", "action ຕ້ອງເປັນ ອະນຸມັດ (approve) ຫຼື ປະຕິເສດ (reject)"),
        ("a reason is required when rejecting, so the seller knows what to fix", "ກະລຸນາລະບຸເຫດຜົນເມື່ອປະຕິເສດ ເພື່ອໃຫ້ຜູ້ຂາຍຮູ້ວ່າຕ້ອງແກ້ໄຂຫຍັງ"),
        ("product not found or not submitted for review", "ບໍ່ພົບສິນຄ້າ ຫຼື ສິນຄ້າຍັງບໍ່ໄດ້ສົ່ງກວດ"),
        ("select between 1 and 100 products", "ກະລຸນາເລືອກສິນຄ້າ 1 ຫາ 100 ລາຍການ"),
        ("select between 1 and 200 items", "ກະລຸນາເລືອກ 1 ຫາ 200 ລາຍການ"),
        ("ids required", "ກະລຸນາລະບຸ ids"),
        // partners / commissions
        ("cannot partner with your own shop", "ບໍ່ສາມາດເປັນຄູ່ຮ່ວມກັບຮ້ານຂອງຕົນເອງ"),
        ("commission_bps must be 0..9000", "ຄ່ານາຍໜ້າ (commission_bps) ຕ້ອງຢູ່ລະຫວ່າງ 0..9000"),
        ("commission_bps must be 0..9000 (0-90%)", "ຄ່ານາຍໜ້າ (commission_bps) ຕ້ອງຢູ່ລະຫວ່າງ 0..9000 (0-90%)"),
        ("only approved commissions (order completed) can be marked paid", "ໝາຍວ່າຈ່າຍແລ້ວໄດ້ສະເພາະຄ່ານາຍໜ້າທີ່ອະນຸມັດແລ້ວ (ຄຳສັ່ງຊື້ສຳເລັດ) ເທົ່ານັ້ນ"),
        ("product is not open for resale or your partnership with the supplier is not approved", "ສິນຄ້ານີ້ບໍ່ໄດ້ເປີດໃຫ້ຂາຍຕໍ່ ຫຼື ການເປັນຄູ່ຮ່ວມກັບຜູ້ສະໜອງຍັງບໍ່ໄດ້ຮັບການອະນຸມັດ"),
        // orders / checkout
        ("cart is empty", "ກະຕ່າຍັງຫວ່າງຢູ່"),
        ("qty must be between 1 and 100", "ຈຳນວນຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 100"),
        ("qty must be between 1 and 10000", "ຈຳນວນຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 10000"),
        ("qty must be 0–999", "ຈຳນວນຕ້ອງຢູ່ລະຫວ່າງ 0–999"),
        ("shipping cannot be negative", "ຄ່າຈັດສົ່ງຕ້ອງບໍ່ຕິດລົບ"),
        ("only open or confirmed orders can be edited", "ແກ້ໄຂໄດ້ສະເພາະຄຳສັ່ງຊື້ທີ່ເປີດຢູ່ ຫຼື ຢືນຢັນແລ້ວເທົ່ານັ້ນ"),
        ("this order can no longer be changed", "ຄຳສັ່ງຊື້ນີ້ບໍ່ສາມາດປ່ຽນແປງໄດ້ອີກແລ້ວ"),
        ("confirm your shipping details first", "ກະລຸນາຢືນຢັນຂໍ້ມູນການຈັດສົ່ງກ່ອນ"),
        ("please fill in your name, phone and full address", "ກະລຸນາໃສ່ຊື່, ເບີໂທ ແລະ ທີ່ຢູ່ໃຫ້ຄົບຖ້ວນ"),
        // POS
        ("add at least one item", "ກະລຸນາເພີ່ມຢ່າງໜ້ອຍ 1 ລາຍການ"),
        ("add a payment", "ກະລຸນາເພີ່ມການຊຳລະເງິນ"),
        ("custom items need a name", "ລາຍການກຳນົດເອງຕ້ອງມີຊື່"),
        ("custom items need a price", "ລາຍການກຳນົດເອງຕ້ອງມີລາຄາ"),
        ("bill discount is larger than the subtotal", "ສ່ວນຫຼຸດທ້າຍບິນຫຼາຍກວ່າຍອດລວມຍ່ອຍ"),
        ("payment amounts must be positive", "ຈຳນວນເງິນທີ່ຊຳລະຕ້ອງຫຼາຍກວ່າ 0"),
        ("card/transfer/QR payments cannot exceed the amount due", "ການຊຳລະດ້ວຍບັດ/ໂອນ/QR ຕ້ອງບໍ່ເກີນຍອດທີ່ຕ້ອງຈ່າຍ"),
        ("enter the transfer reference or slip number", "ກະລຸນາໃສ່ເລກອ້າງອີງການໂອນ ຫຼື ເລກສະລິບ"),
        ("give a reason for voiding", "ກະລຸນາລະບຸເຫດຜົນການຍົກເລີກ"),
        ("sale is already voided", "ການຂາຍນີ້ຖືກຍົກເລີກແລ້ວ"),
        ("cannot invoice a voided sale", "ບໍ່ສາມາດອອກໃບແຈ້ງໜີ້ໃຫ້ການຂາຍທີ່ຖືກຍົກເລີກແລ້ວ"),
        ("a tax invoice needs the customer's name and address", "ໃບແຈ້ງໜີ້ອາກອນຕ້ອງມີຊື່ ແລະ ທີ່ຢູ່ຂອງລູກຄ້າ"),
        ("a tax invoice was already issued for this sale — reprint it instead", "ການຂາຍນີ້ອອກໃບແຈ້ງໜີ້ອາກອນແລ້ວ — ກະລຸນາພິມຄືນແທນ"),
        // content / ads / media
        ("title and body are required", "ກະລຸນາໃສ່ຫົວຂໍ້ ແລະ ເນື້ອໃນ"),
        ("status must be draft or published", "ສະຖານະຕ້ອງເປັນ ສະບັບຮ່າງ (draft) ຫຼື ເຜີຍແຜ່ແລ້ວ (published)"),
        ("headline and a positive budget are required", "ກະລຸນາໃສ່ຫົວຂໍ້ ແລະ ງົບປະມານທີ່ຫຼາຍກວ່າ 0"),
        ("status must be draft, active, paused or ended", "ສະຖານະຕ້ອງເປັນ ສະບັບຮ່າງ (draft), ກຳລັງດຳເນີນ (active), ຢຸດຊົ່ວຄາວ (paused) ຫຼື ສິ້ນສຸດ (ended)"),
        ("you can only advertise your own or resold products", "ທ່ານໂຄສະນາໄດ້ສະເພາະສິນຄ້າຂອງຕົນເອງ ຫຼື ສິນຄ້າທີ່ຂາຍຕໍ່ເທົ່ານັ້ນ"),
        ("kind must be image or video", "ປະເພດຕ້ອງເປັນຮູບພາບ ຫຼື ວິດີໂອ"),
        ("no files received (use the 'file' field)", "ບໍ່ໄດ້ຮັບໄຟລ໌ (ໃຊ້ຊ່ອງ 'file')"),
        ("unsupported file type — use JPEG, PNG, WebP, GIF, MP4, WebM or MOV", "ບໍ່ຮອງຮັບໄຟລ໌ປະເພດນີ້ — ກະລຸນາໃຊ້ JPEG, PNG, WebP, GIF, MP4, WebM ຫຼື MOV"),
        ("url must be an http(s) link", "url ຕ້ອງເປັນລິ້ງ http(s)"),
        ("some media items are not in this shop's library", "ບາງລາຍການບໍ່ຢູ່ໃນຄັງສື່ຂອງຮ້ານນີ້"),
        // AI
        ("AI is not configured", "ຍັງບໍ່ໄດ້ຕັ້ງຄ່າ AI"),
        ("AI returned no choices", "AI ບໍ່ໄດ້ສົ່ງຄຳຕອບກັບມາ"),
        ("last message must be from the user", "ຂໍ້ຄວາມສຸດທ້າຍຕ້ອງມາຈາກຜູ້ໃຊ້"),
        // social commerce
        ("hold time must be 1–336 hours", "ເວລາຈອງຕ້ອງຢູ່ລະຫວ່າງ 1–336 ຊົ່ວໂມງ"),
        ("max quantity must be 1–999", "ຈຳນວນສູງສຸດຕ້ອງຢູ່ລະຫວ່າງ 1–999"),
        ("enter the Facebook Page ID", "ກະລຸນາໃສ່ Facebook Page ID"),
        ("enter the WhatsApp phone number ID", "ກະລຸນາໃສ່ WhatsApp phone number ID"),
        ("enter the TikTok account open_id", "ກະລຸນາໃສ່ open_id ຂອງບັນຊີ TikTok"),
        ("that account is already connected (possibly by another shop)", "ບັນຊີນີ້ຖືກເຊື່ອມຕໍ່ແລ້ວ (ອາດຈະໂດຍຮ້ານອື່ນ)"),
        ("`message` and `user_id` are required", "ຕ້ອງລະບຸ `message` ແລະ `user_id`"),
        ("type a comment", "ກະລຸນາພິມຄອມເມັ້ນ"),
        // shop staff
        ("only the shop owner can change the shop type", "ສະເພາະເຈົ້າຂອງຮ້ານເທົ່ານັ້ນທີ່ປ່ຽນປະເພດຮ້ານໄດ້"),
        ("unknown permission", "ບໍ່ຮູ້ຈັກສິດນີ້"),
        ("give the role a name", "ກະລຸນາຕັ້ງຊື່ບົດບາດ"),
        ("a shop can have at most 30 roles", "ຮ້ານໜຶ່ງມີບົດບາດໄດ້ສູງສຸດ 30 ບົດບາດ"),
        ("a role with this name already exists", "ມີບົດບາດຊື່ນີ້ແລ້ວ"),
        ("choose one of this shop's roles", "ກະລຸນາເລືອກບົດບາດຂອງຮ້ານນີ້"),
        ("too many open invites — revoke unused links first", "ມີລິ້ງເຊີນທີ່ຍັງເປີດຢູ່ຫຼາຍເກີນໄປ — ກະລຸນາຍົກເລີກລິ້ງທີ່ບໍ່ໄດ້ໃຊ້ກ່ອນ"),
        ("this invite link has expired — ask the shop for a new one", "ລິ້ງເຊີນນີ້ໝົດອາຍຸແລ້ວ — ກະລຸນາຂໍລິ້ງໃໝ່ຈາກຮ້ານ"),
        ("this invite link is no longer valid — ask the shop for a new one", "ລິ້ງເຊີນນີ້ໃຊ້ບໍ່ໄດ້ແລ້ວ — ກະລຸນາຂໍລິ້ງໃໝ່ຈາກຮ້ານ"),
        ("you own this shop", "ທ່ານເປັນເຈົ້າຂອງຮ້ານນີ້ແລ້ວ"),
        // POS open bills
        ("bill name can be at most 40 characters", "ຊື່ບິນຍາວໄດ້ບໍ່ເກີນ 40 ຕົວອັກສອນ"),
        ("this bill is too large", "ບິນນີ້ໃຫຍ່ເກີນໄປ"),
        ("version is required", "ຕ້ອງລະບຸເວີຊັນ (version)"),
        ("this bill was changed on another till — reload it", "ບິນນີ້ຖືກແກ້ໄຂຢູ່ເຄື່ອງຂາຍອື່ນ — ກະລຸນາໂຫຼດໃໝ່"),
    ])
});

/// Messages built with `format!`: regex over the final string → Lao template (`$1`, `$2`…).
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"^'(.+)' belongs to another shop$", "'$1' ເປັນຂອງຮ້ານອື່ນ"),
        (r"^'(.+)' has only (-?\d+) left$", "'$1' ເຫຼືອພຽງ $2"),
        (r"^'(.+)' is archived$", "'$1' ຢຸດຂາຍແລ້ວ"),
        (r"^'(.+)' is no longer sold by that shop$", "'$1' ບໍ່ມີຂາຍຢູ່ຮ້ານນັ້ນແລ້ວ"),
        (r"^'(.+)' is not available$", "'$1' ບໍ່ພ້ອມຈຳໜ່າຍ"),
        (r"^only (-?\d+) × '(.+)' in stock$", "'$2' ມີໃນສະຕັອກພຽງ $1"),
        (r"^only (-?\d+) more in stock$", "ສະຕັອກເຫຼືອອີກພຽງ $1"),
        (r"^discount on '(.+)' is larger than the line$", "ສ່ວນຫຼຸດຂອງ '$1' ຫຼາຍກວ່າຍອດຂອງລາຍການ"),
        (r"^at most (\d+) lines per sale$", "ການຂາຍໜຶ່ງຄັ້ງມີໄດ້ສູງສຸດ $1 ລາຍການ"),
        (r"^payment is short by (.+)$", "ການຊຳລະເງິນຍັງຂາດອີກ $1"),
        (r"^image is too small \((\d+)×(\d+) px\) — use at least (\d+) px on the shortest side$", "ຮູບນ້ອຍເກີນໄປ ($1×$2 px) — ດ້ານສັ້ນສຸດຕ້ອງມີຢ່າງໜ້ອຍ $3 px"),
        (r"^media read failed: (.+)$", "ອ່ານໄຟລ໌ສື່ບໍ່ສຳເລັດ: $1"),
        (r"^cannot mark a (\w+) COD order as (\w+)$", "ບໍ່ສາມາດປ່ຽນຄຳສັ່ງຊື້ COD ທີ່ເປັນ $1 ເປັນ $2 ໄດ້"),
        (r"^cannot return an order that is (\w+)$", "ບໍ່ສາມາດຕີກັບຄຳສັ່ງຊື້ທີ່ເປັນ $1 ໄດ້"),
        (r"^choose a delivery option for (.+)$", "ກະລຸນາເລືອກວິທີຈັດສົ່ງສຳລັບ $1"),
        (r"^at most (\d+) open bills — finish or cancel some first$", "ເປີດບິນໄດ້ສູງສຸດ $1 ບິນ — ກະລຸນາປິດ ຫຼື ຍົກເລີກບາງບິນກ່ອນ"),
        (r"^(\d+) staff member\(s\) have this role — give them another role first$", "ມີພະນັກງານ $1 ຄົນໃຊ້ບົດບາດນີ້ — ກະລຸນາປ່ຽນບົດບາດໃຫ້ເຂົາເຈົ້າກ່ອນ"),
        (r"^please wait (\d+) seconds before requesting another code$", "ກະລຸນາລໍຖ້າ $1 ວິນາທີ ກ່ອນຂໍລະຫັດໃໝ່"),
        (r"^could not send the WhatsApp message: (.+)$", "ສົ່ງຂໍ້ຄວາມ WhatsApp ບໍ່ສຳເລັດ: $1"),
        (r"^payment method must be one of (.+)$", "ວິທີຊຳລະເງິນຕ້ອງເປັນໜຶ່ງໃນ $1"),
        (r"^product (\S+) not found$", "ບໍ່ພົບສິນຄ້າ $1"),
        (r"^order is (.+)$", "ຄຳສັ່ງຊື້ນີ້ຢູ່ໃນສະຖານະ $1 ແລ້ວ"),
        (r"^cannot cancel an order that is (.+)$", "ບໍ່ສາມາດຍົກເລີກຄຳສັ່ງຊື້ທີ່ຢູ່ໃນສະຖານະ $1"),
        (r"^cannot move a (.+?) order to (.+)$", "ບໍ່ສາມາດປ່ຽນຄຳສັ່ງຊື້ຈາກສະຖານະ $1 ເປັນ $2"),
        (r"^cannot move order from (.+?) to (.+)$", "ບໍ່ສາມາດປ່ຽນຄຳສັ່ງຊື້ຈາກສະຖານະ $1 ເປັນ $2"),
        (r"^categories can be nested at most (\d+) levels deep$", "ໝວດໝູ່ຊ້ອນກັນໄດ້ສູງສຸດ $1 ຊັ້ນ"),
        (r"^has (\d+) sub-categor\w* — move or delete them first$", "ມີໝວດໝູ່ຍ່ອຍ $1 ໝວດ — ກະລຸນາຍ້າຍ ຫຼື ລຶບອອກກ່ອນ"),
        (
            r"^(\d+) product\(s\) use this category — reassign them or deactivate the category instead$",
            "ມີສິນຄ້າ $1 ລາຍການໃຊ້ໝວດໝູ່ນີ້ — ກະລຸນາຍ້າຍສິນຄ້າໄປໝວດໝູ່ອື່ນ ຫຼື ປິດການໃຊ້ງານໝວດໝູ່ນີ້ແທນ",
        ),
        (r"^constraint violated: (.*)$", "ຂໍ້ມູນບໍ່ຜ່ານເງື່ອນໄຂ: $1"),
        (r"^kind must be one of (.+)$", "ປະເພດ (kind) ຕ້ອງເປັນໜຶ່ງໃນ $1"),
        (r"^provider must be one of (.+)$", "ຊ່ອງທາງ (provider) ຕ້ອງເປັນໜຶ່ງໃນ $1"),
        // media
        (r"^a product can have at most (\d+) media items$", "ສິນຄ້າໜຶ່ງມີສື່ໄດ້ສູງສຸດ $1 ລາຍການ"),
        (r"^at most (\d+) files per upload$", "ອັບໂຫຼດໄດ້ສູງສຸດ $1 ໄຟລ໌ຕໍ່ຄັ້ງ"),
        (r"^gallery is limited to (\d+) items — files were saved to your library$", "ແກລເລີຣີມີໄດ້ສູງສຸດ $1 ລາຍການ — ໄຟລ໌ຖືກບັນທຶກໄວ້ໃນຄັງສື່ແລ້ວ"),
        (r"^image is larger than (\d+) MB$", "ຮູບພາບໃຫຍ່ເກີນ $1 MB"),
        (r"^video is larger than (\d+) MB$", "ວິດີໂອໃຫຍ່ເກີນ $1 MB"),
        (
            r"^in use by (\d+) product\(s\), (\d+) content item\(s\) and (\d+) ad\(s\) — delete with force to remove it everywhere$",
            "ຖືກໃຊ້ຢູ່ໃນສິນຄ້າ $1 ລາຍການ, ຄອນເທັນ $2 ລາຍການ ແລະ ໂຄສະນາ $3 ລາຍການ — ລຶບແບບບັງຄັບ (force) ເພື່ອລຶບອອກຈາກທຸກບ່ອນ",
        ),
        (r"^invalid upload: (.*)$", "ການອັບໂຫຼດບໍ່ຖືກຕ້ອງ: $1"),
        (r"^unreadable image: (.*)$", "ອ່ານຮູບພາບບໍ່ໄດ້: $1"),
        (r"^upload interrupted or too large: (.*)$", "ການອັບໂຫຼດຖືກຂັດຈັງຫວະ ຫຼື ໄຟລ໌ໃຫຍ່ເກີນ: $1"),
        (r"^media upload failed: (.*)$", "ອັບໂຫຼດສື່ບໍ່ສຳເລັດ: $1"),
        // AI
        (r"^AI error \((.+?)\): (.*)$", "AI ຜິດພາດ ($1): $2"),
        (r"^AI request failed: (.*)$", "ສົ່ງຄຳຮ້ອງໄປຫາ AI ບໍ່ສຳເລັດ: $1"),
        (r"^AI response: (.*)$", "ຄຳຕອບຈາກ AI ບໍ່ຖືກຕ້ອງ: $1"),
        (r"^AI response \((.+?)\): (.*)$", "ຄຳຕອບຈາກ AI ບໍ່ຖືກຕ້ອງ ($1): $2"),
        // axum extractor rejections (plain-text bodies)
        (r"^Failed to parse the request body as JSON: (.*)$", "ຂໍ້ມູນ JSON ບໍ່ຖືກຕ້ອງ: $1"),
        (r"^Failed to deserialize the JSON body into the target type: (.*)$", "ຂໍ້ມູນທີ່ສົ່ງມາບໍ່ຖືກຕ້ອງ: $1"),
        (r"^Expected request with `Content-Type: application/json`$", "ຄຳຮ້ອງຕ້ອງມີ `Content-Type: application/json`"),
        (r"^Failed to buffer the request body: length limit exceeded$", "ຂໍ້ມູນທີ່ສົ່ງມາໃຫຍ່ເກີນກຳນົດ"),
        (r"^Failed to buffer the request body: (.*)$", "ອ່ານຂໍ້ມູນທີ່ສົ່ງມາບໍ່ໄດ້: $1"),
        (r"^Failed to deserialize query string: (.*)$", "ພາຣາມິເຕີ query ບໍ່ຖືກຕ້ອງ: $1"),
        (r"^Invalid URL: (.*)$", "URL ບໍ່ຖືກຕ້ອງ: $1"),
        (r"^(?:Invalid|Error parsing) .*`multipart/form-data` request.*$", "ຂໍ້ມູນ multipart/form-data ບໍ່ຖືກຕ້ອງ"),
    ]
    .into_iter()
    .map(|(re, tpl)| (Regex::new(re).expect("valid i18n regex"), tpl))
    .collect()
});

/// Translations registered by cores at startup (each core keeps its own dictionary).
static EXTRA: RwLock<Extra> = RwLock::new(Extra { exact: Vec::new(), patterns: Vec::new(), fns: Vec::new() });

struct Extra {
    exact: Vec<(&'static str, &'static str)>,
    patterns: Vec<(Regex, &'static str)>,
    /// Translator functions (e.g. commerce's plug-in modules).
    fns: Vec<fn(&str) -> Option<String>>,
}

/// Add a core's Lao dictionary: exact messages and regex patterns (`$1` keeps dynamic parts).
pub fn register_lao(exact: &'static [(&'static str, &'static str)], patterns: &'static [(&'static str, &'static str)]) {
    let mut x = EXTRA.write().expect("i18n lock");
    x.exact.extend_from_slice(exact);
    x.patterns.extend(patterns.iter().map(|(re, tpl)| (Regex::new(re).expect("valid i18n regex"), *tpl)));
}

/// Add a translator function (checked after the dictionaries).
pub fn register_lao_fn(f: fn(&str) -> Option<String>) {
    EXTRA.write().expect("i18n lock").fns.push(f);
}

/// Lao translation of an English API message, if one is known.
pub fn translate_lo(msg: &str) -> Option<String> {
    let msg = msg.trim();
    if let Some(t) = EXACT.get(msg) {
        return Some((*t).to_string());
    }
    if let Some((re, tpl)) = PATTERNS.iter().find(|(re, _)| re.is_match(msg)) {
        return Some(re.replace(msg, *tpl).into_owned());
    }
    let x = EXTRA.read().expect("i18n lock");
    if let Some((_, t)) = x.exact.iter().find(|(k, _)| *k == msg) {
        return Some((*t).to_string());
    }
    if let Some((re, tpl)) = x.patterns.iter().find(|(re, _)| re.is_match(msg)) {
        return Some(re.replace(msg, *tpl).into_owned());
    }
    x.fns.iter().find_map(|f| f(msg))
}

/// True when the first language range of `Accept-Language` has primary tag `lo`.
pub fn wants_lao(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|r| r.split(';').next())
        .and_then(|tag| tag.trim().split(['-', '_']).next())
        .is_some_and(|p| p.eq_ignore_ascii_case("lo"))
}

fn error_code(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        413 => "payload_too_large",
        415 => "unsupported_media_type",
        422 => "unprocessable_entity",
        s if s >= 500 => "internal",
        _ => "error",
    }
}

/// Middleware: translate error `message`s to Lao for `Accept-Language: lo` requests.
pub async fn localize_errors(req: Request, next: Next) -> Response {
    let lao = wants_lao(req.headers());
    let res = next.run(req).await;
    if !lao || res.status().as_u16() < 400 {
        return res;
    }
    let ct = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_json = ct.starts_with("application/json");
    let is_text = ct.starts_with("text/plain");
    if !is_json && !is_text {
        return res;
    }
    let too_big = res
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > MAX_ERROR_BODY);
    if too_big {
        return res;
    }

    let (mut parts, body) = res.into_parts();
    let bytes = match to_bytes(body, MAX_ERROR_BODY).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    let new_body: Option<Vec<u8>> = if is_json {
        serde_json::from_slice::<Value>(&bytes).ok().and_then(|mut v| {
            let lo = v.get("message").and_then(Value::as_str).and_then(translate_lo)?;
            v["message"] = Value::String(lo);
            serde_json::to_vec(&v).ok()
        })
    } else {
        // Framework rejections are plain text; for Lao clients turn them into the usual JSON shape.
        std::str::from_utf8(&bytes).ok().and_then(translate_lo).and_then(|lo| {
            parts.headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            serde_json::to_vec(&json!({ "error": error_code(parts.status), "message": lo })).ok()
        })
    };

    let out = new_body.map(bytes::Bytes::from).unwrap_or(bytes);
    parts.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(out.len()));
    Response::from_parts(parts, Body::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_examples() {
        assert_eq!(translate_lo("cart is empty").unwrap(), "ກະຕ່າຍັງຫວ່າງຢູ່");
        assert_eq!(translate_lo("invalid email or password").unwrap(), "ອີເມວ ຫຼື ລະຫັດຜ່ານບໍ່ຖືກຕ້ອງ");
        assert_eq!(translate_lo("not found").unwrap(), "ບໍ່ພົບຂໍ້ມູນ");
        assert_eq!(translate_lo("enter the Facebook Page ID").unwrap(), "ກະລຸນາໃສ່ Facebook Page ID");
        assert!(translate_lo("some brand new message").is_none());
    }

    #[test]
    fn pattern_examples_keep_dynamic_parts() {
        assert_eq!(translate_lo("only 3 × 'Red Shirt' in stock").unwrap(), "'Red Shirt' ມີໃນສະຕັອກພຽງ 3");
        assert_eq!(translate_lo("'Mug' has only 2 left").unwrap(), "'Mug' ເຫຼືອພຽງ 2");
        assert_eq!(translate_lo("image is larger than 10 MB").unwrap(), "ຮູບພາບໃຫຍ່ເກີນ 10 MB");
        assert_eq!(translate_lo("has 1 sub-category — move or delete them first").unwrap(), "ມີໝວດໝູ່ຍ່ອຍ 1 ໝວດ — ກະລຸນາຍ້າຍ ຫຼື ລຶບອອກກ່ອນ");
        assert_eq!(
            translate_lo("cannot move a pending order to shipped").unwrap(),
            "ບໍ່ສາມາດປ່ຽນຄຳສັ່ງຊື້ຈາກສະຖານະ pending ເປັນ shipped"
        );
    }

    #[test]
    fn accept_language_detection() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_str(v).unwrap());
            m
        };
        assert!(wants_lao(&h("lo")));
        assert!(wants_lao(&h("lo-LA,en;q=0.8")));
        assert!(wants_lao(&h("LO")));
        assert!(!wants_lao(&h("en")));
        assert!(!wants_lao(&h("en-US,lo;q=0.5")));
        assert!(!wants_lao(&HeaderMap::new()));
    }

    #[tokio::test]
    async fn middleware_translates_json_and_text_errors_only_for_lao() {
        use axum::{http::StatusCode, routing::get, Router};
        use tower::ServiceExt;

        let app = Router::new()
            .route("/e", get(|| async { crate::error::AppError::bad("cart is empty") }))
            .route("/nf", get(|| async { crate::error::AppError::NotFound }))
            .route("/t", get(|| async { (StatusCode::UNPROCESSABLE_ENTITY, "Failed to deserialize the JSON body into the target type: missing field `x`") }))
            .route("/ok", get(|| async { "cart is empty" }))
            .layer(axum::middleware::from_fn(localize_errors));

        async fn call(app: &Router, path: &str, lang: &str) -> (StatusCode, HeaderMap, String) {
            let req = Request::builder().uri(path).header("accept-language", lang).body(Body::empty()).unwrap();
            let res = app.clone().oneshot(req).await.unwrap();
            let (p, b) = res.into_parts();
            (p.status, p.headers, String::from_utf8(to_bytes(b, 1 << 20).await.unwrap().to_vec()).unwrap())
        }

        let (s, h, b) = call(&app, "/e", "lo").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let v: Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["message"], "ກະຕ່າຍັງຫວ່າງຢູ່");
        assert_eq!(v["error"], "bad_request");
        assert_eq!(h[header::CONTENT_LENGTH].to_str().unwrap(), b.len().to_string());

        let (_, _, b) = call(&app, "/nf", "lo-LA").await;
        assert_eq!(serde_json::from_str::<Value>(&b).unwrap()["message"], "ບໍ່ພົບຂໍ້ມູນ");

        let (_, _, b) = call(&app, "/e", "en").await;
        assert_eq!(serde_json::from_str::<Value>(&b).unwrap()["message"], "cart is empty");

        let (s, h, b) = call(&app, "/t", "lo").await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(h[header::CONTENT_TYPE], "application/json");
        assert_eq!(serde_json::from_str::<Value>(&b).unwrap()["message"], "ຂໍ້ມູນທີ່ສົ່ງມາບໍ່ຖືກຕ້ອງ: missing field `x`");

        let (_, h, b) = call(&app, "/t", "en").await;
        assert!(h[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/plain"));
        assert!(b.starts_with("Failed to deserialize"));

        let (s, _, b) = call(&app, "/ok", "lo").await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, "cart is empty");
    }
}
