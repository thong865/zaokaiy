//! Lao translations of this core's error messages.

pub const EXACT: &[(&str, &str)] = &[
    ("enter the sum insured", "ກະລຸນາໃສ່ທຶນປະກັນ"),
    ("enter the applicant's full name", "ກະລຸນາໃສ່ຊື່ ແລະ ນາມສະກຸນຂອງຜູ້ສະໝັກ"),
    ("enter a valid phone number", "ກະລຸນາໃສ່ເບີໂທທີ່ຖືກຕ້ອງ"),
    ("enter a valid email address", "ກະລຸນາໃສ່ອີເມວທີ່ຖືກຕ້ອງ"),
    ("start date must be within the next 6 months", "ວັນເລີ່ມຕ້ອງຢູ່ພາຍໃນ 6 ເດືອນຂ້າງໜ້າ"),
    ("too many or too long details", "ລາຍລະອຽດຫຼາຍ ຫຼື ຍາວເກີນໄປ"),
    ("choose an insurance type", "ກະລຸນາເລືອກປະເພດປະກັນໄພ"),
    ("enter the insurer's name", "ກະລຸນາໃສ່ຊື່ບໍລິສັດປະກັນໄພ"),
    ("enter a plan name (up to 120 characters)", "ກະລຸນາໃສ່ຊື່ແຜນ (ບໍ່ເກີນ 120 ຕົວອັກສອນ)"),
    ("up to 20 coverage points, 160 characters each", "ຄວາມຄຸ້ມຄອງບໍ່ເກີນ 20 ຂໍ້, ຂໍ້ລະ 160 ຕົວອັກສອນ"),
    ("set the premium", "ກະລຸນາກຳນົດຄ່າທຳນຽມປະກັນ"),
    ("set the premium rate", "ກະລຸນາກຳນົດອັດຕາຄ່າທຳນຽມປະກັນ"),
    ("premium must be fixed or a rate of the sum insured", "ຄ່າທຳນຽມຕ້ອງເປັນຈຳນວນຄົງທີ່ ຫຼື ເປັນອັດຕາຂອງທຶນປະກັນ"),
    ("amounts can't be negative", "ຈຳນວນເງິນຕິດລົບບໍ່ໄດ້"),
    ("maximum sum insured must be at least the minimum", "ທຶນປະກັນສູງສຸດຕ້ອງບໍ່ໜ້ອຍກວ່າຕ່ຳສຸດ"),
    ("set the maximum sum insured", "ກະລຸນາກຳນົດທຶນປະກັນສູງສຸດ"),
    ("term must be between 1 and 120 months", "ໄລຍະເວລາຕ້ອງຢູ່ລະຫວ່າງ 1 ຫາ 120 ເດືອນ"),
    ("only insurance agent shops can publish plans", "ສະເພາະຮ້ານຕົວແທນປະກັນໄພເທົ່ານັ້ນທີ່ເຜີຍແຜ່ແຜນໄດ້"),
    ("this application is closed", "ໃບສະໝັກນີ້ປິດແລ້ວ"),
    ("mark the premium as paid before issuing the policy", "ກະລຸນາໝາຍວ່າຈ່າຍຄ່າທຳນຽມແລ້ວ ກ່ອນອອກກົມມະທຳ"),
    ("enter the policy number from the insurer", "ກະລຸນາໃສ່ເລກກົມມະທຳຈາກບໍລິສັດປະກັນໄພ"),
    ("add a note for the customer", "ກະລຸນາເພີ່ມໝາຍເຫດໃຫ້ລູກຄ້າ"),
];

pub const PATTERNS: &[(&str, &str)] = &[
    (r"^sum insured must be between ([\d,.]+) and ([\d,.]+)$", "ທຶນປະກັນຕ້ອງຢູ່ລະຫວ່າງ $1 ຫາ $2"),
    (r"^can't (\w+) an application that is (\w+)$", "ບໍ່ສາມາດ $1 ໃບສະໝັກທີ່ມີສະຖານະ $2"),
];
