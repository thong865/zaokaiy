//! Lao translations of this core's error messages.

pub const EXACT: &[(&str, &str)] = &[
    ("this restaurant isn't taking orders right now", "ຮ້ານອາຫານນີ້ຍັງບໍ່ຮັບອໍເດີໃນຕອນນີ້"),
    ("this order type isn't available here", "ຮ້ານນີ້ບໍ່ມີການສັ່ງແບບນີ້"),
    ("scan the QR code on your table to order", "ກະລຸນາສະແກນ QR ໂຄດທີ່ໂຕະຂອງທ່ານເພື່ອສັ່ງອາຫານ"),
    ("enter a phone number so the restaurant can reach you", "ກະລຸນາໃສ່ເບີໂທເພື່ອໃຫ້ຮ້ານຕິດຕໍ່ທ່ານໄດ້"),
    ("enter a delivery address", "ກະລຸນາໃສ່ທີ່ຢູ່ສົ່ງ"),
    ("add at least one dish", "ກະລຸນາເພີ່ມອາຫານຢ່າງໜ້ອຍ 1 ລາຍການ"),
    ("quantity must be between 1 and 99", "ຈຳນວນຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 99"),
    ("a dish on your order is no longer on the menu", "ມີອາຫານໃນອໍເດີຂອງທ່ານທີ່ບໍ່ມີໃນເມນູແລ້ວ"),
    ("service charge must be between 0% and 30%", "ຄ່າບໍລິການຕ້ອງຢູ່ລະຫວ່າງ 0% ຫາ 30%"),
    ("preparation time must be between 0 and 240 minutes", "ເວລາກະກຽມຕ້ອງຢູ່ລະຫວ່າງ 0 ຫາ 240 ນາທີ"),
    ("turn on at least one way to order", "ກະລຸນາເປີດວິທີສັ່ງຢ່າງໜ້ອຍ 1 ແບບ"),
    ("enter a name (up to 80 characters)", "ກະລຸນາໃສ່ຊື່ (ບໍ່ເກີນ 80 ຕົວອັກສອນ)"),
    ("price can't be negative", "ລາຄາຕິດລົບບໍ່ໄດ້"),
    ("spice level must be 0 to 3", "ລະດັບຄວາມເຜັດຕ້ອງເປັນ 0 ຫາ 3"),
    ("image must be a web address (http/https)", "ຮູບຕ້ອງເປັນລິ້ງເວັບ (http/https)"),
    ("that menu section belongs to another shop", "ໝວດເມນູນີ້ເປັນຂອງຮ້ານອື່ນ"),
    ("a table with that name already exists", "ມີໂຕະຊື່ນີ້ຢູ່ແລ້ວ"),
    ("seats must be between 1 and 100", "ຈຳນວນບ່ອນນັ່ງຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 100"),
    ("payment method must be cash, transfer or card", "ວິທີຈ່າຍຕ້ອງເປັນ ເງິນສົດ, ໂອນ ຫຼື ບັດ"),
    ("a cancelled order can't be marked paid", "ອໍເດີທີ່ຍົກເລີກແລ້ວ ໝາຍວ່າຈ່າຍແລ້ວບໍ່ໄດ້"),
];

pub const PATTERNS: &[(&str, &str)] = &[
    (r"^'(.+)' is sold out$", "'$1' ໝົດແລ້ວ"),
    (r"^can't move an order from (\w+) to (\w+)$", "ບໍ່ສາມາດປ່ຽນອໍເດີຈາກ $1 ເປັນ $2"),
];
