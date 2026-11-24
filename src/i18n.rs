use crate::models::AppLang;
use crate::rtl_text::shape_arabic;

/// Complete localized UI string table for Arabic (`AppLang::Ar`, RTL) and
/// English (`AppLang::En`, LTR).
#[derive(Clone, Copy)]
pub struct UiStrings {
    pub tab_home: &'static str,
    pub tab_index: &'static str,
    pub tab_harbor: &'static str,
    pub back_btn: &'static str,
    pub page_word: &'static str,

    pub status_done: &'static str,
    pub status_active: &'static str,
    pub status_cancelled: &'static str,
    pub status_total: &'static str,
    pub status_saving: &'static str,
    pub status_auto_saved: &'static str,
    pub status_save_error: &'static str,
    pub toast_undone: &'static str,

    pub branch_placeholder: &'static str,
    pub index_new_pane_placeholder: &'static str,
    pub index_empty_hint: &'static str,
    pub task_title_placeholder: &'static str,
    pub untitled: &'static str,

    pub note_tag: &'static str,
    pub copy_btn: &'static str,
    pub copied_btn: &'static str,
    pub note_placeholder: &'static str,

    pub action_archive: &'static str,
    pub action_delete: &'static str,
    pub action_confirm: &'static str,
    pub action_edit: &'static str,
    pub action_open: &'static str,
    pub action_active_badge: &'static str,
    pub action_restore: &'static str,

    pub ctx_active: &'static str,
    pub ctx_completed: &'static str,
    pub ctx_cancelled: &'static str,
    pub ctx_notes: &'static str,
    pub ctx_edit: &'static str,
    pub ctx_add_branch: &'static str,
    pub ctx_delete_task: &'static str,

    pub profiles_title: &'static str,
    pub profiles_tag: &'static str,
    pub profiles_new: &'static str,
    pub profiles_carry_over: &'static str,
    pub profiles_title_placeholder: &'static str,
    pub profiles_invalid_date: &'static str,

    pub guide_title: &'static str,
    pub guide_bullets: [&'static str; 4],
    pub guide_cta: &'static str,

    pub harbor_title: &'static str,
    pub harbor_empty: &'static str,
    pub harbor_search_placeholder: &'static str,
    pub harbor_no_matches: &'static str,

    pub empty_board_title: &'static str,
    pub empty_board_add: &'static str,

    pub quick_entry_title: &'static str,
    pub quick_entry_hint: &'static str,
    pub quick_entry_placeholder: &'static str,
    pub quick_entry_preview: &'static str,
    pub quick_entry_create: &'static str,
    pub quick_entry_cancel: &'static str,
    pub quick_entry_duplicate: &'static str,

    pub zen_badge: &'static str,

    pub load_error_title: &'static str,
    pub load_error_intro: &'static str,
    pub load_error_details_label: &'static str,
    pub load_error_backup_label: &'static str,
    pub load_error_backup_failed_label: &'static str,
    pub load_error_copy_path: &'static str,
    pub load_error_copied_path: &'static str,
    pub load_error_start_fresh: &'static str,
    pub load_error_exit: &'static str,

    pub data_io_btn: &'static str,
    pub data_io_title: &'static str,
    pub data_io_export_json: &'static str,
    pub data_io_export_json_desc: &'static str,
    pub data_io_export_md: &'static str,
    pub data_io_export_md_desc: &'static str,
    pub data_io_import_json: &'static str,
    pub data_io_import_json_desc: &'static str,
    pub data_io_confirm_title: &'static str,
    pub data_io_confirm_warning: &'static str,
    pub data_io_confirm_btn: &'static str,
    pub data_io_waiting_dialog: &'static str,

    pub tip_close: &'static str,
    pub tip_maximize: &'static str,
    pub tip_minimize: &'static str,
    pub tip_switch_lang: &'static str,
    pub tip_prev_page: &'static str,
    pub tip_next_page: &'static str,
    pub tip_page_cycle: &'static str,
    pub tip_close_esc: &'static str,
    pub tip_toggle_complete: &'static str,
    pub tip_expand_branch: &'static str,
    pub tip_collapse_branch: &'static str,
    pub tip_remove_staged: &'static str,
}

const AR_STRINGS: UiStrings = UiStrings {
    tab_home: "الرئيسية",
    tab_index: "الفهرس",
    tab_harbor: "الأرشيف",
    back_btn: "→ رجوع [Esc]",
    page_word: "صفحة",

    status_done: "مكتمل",
    status_active: "نشط",
    status_cancelled: "ملغى",
    status_total: "المجموع",
    status_saving: "جاري الحفظ...",
    status_auto_saved: "محفوظ تلقائياً",
    status_save_error: "فشل الحفظ",
    toast_undone: "تم التراجع (Ctrl+Z)",

    branch_placeholder: "+ اكتب فرعاً جديداً...",
    index_new_pane_placeholder: "+ اسم المهمة الرئيسية الجديدة...",
    index_empty_hint: "لا توجد لوحات رئيسية بعد — اكتب أعلاه واضغط Enter",
    task_title_placeholder: "عنوان المهمة...",
    untitled: "بدون عنوان",

    note_tag: "ملاحظات",
    copy_btn: "نسخ",
    copied_btn: "تم النسخ",
    note_placeholder: "اكتب ملاحظاتك هنا...",

    action_archive: "أرشفة",
    action_delete: "حذف",
    action_confirm: "تأكيد؟",
    action_edit: "تعديل",
    action_open: "تفعيل",
    action_active_badge: "نشط",
    action_restore: "استعادة",

    ctx_active: "نشط [1]",
    ctx_completed: "مكتمل [2]",
    ctx_cancelled: "ملغى [3]",
    ctx_notes: "الملاحظات",
    ctx_edit: "تعديل",
    ctx_add_branch: "إضافة فرع",
    ctx_delete_task: "حذف المهمة",

    profiles_title: "البروفايلات اليومية",
    profiles_tag: "PROFILES",
    profiles_new: "+ بروفايل جديد",
    profiles_carry_over: "ترحيل النشط من الأمس",
    profiles_title_placeholder: "عنوان اليوم...",
    profiles_invalid_date: "صيغة التاريخ يجب أن تكون YYYY-MM-DD",

    guide_title: "دليل الاستخدام السريع",
    guide_bullets: [
        "1. إضافة الفروع: اكتب في الشريط السفلي لأي لوحة واضغط Enter.",
        "2. طيّ الفروع: اضغط المربع الصغير [−] أو [+N] يسار البطاقة لإخفاء أو إظهار تفرعاتها.",
        "3. الملاحظات: نقرة واحدة على أي بطاقة تفتح دفتر الملاحظات الورقي الخاص بها.",
        "4. تنظيم الصفحات: افتح تبويب الفهرس وحدد أي مهمة لتحريكها بين الصفحات بالأسهم ↑ ↓.",
    ],
    guide_cta: "ابدأ العمل",

    harbor_title: "الأرشيف",
    harbor_empty: "لا توجد لوحات مؤرشفة بعد.",
    harbor_search_placeholder: "ابحث في اللوحات المؤرشفة والمهام والملاحظات...",
    harbor_no_matches: "لا توجد لوحات مؤرشفة مطابقة للبحث.",

    empty_board_title: "لا توجد مهام بعد",
    empty_board_add: "+ إضافة مهام",

    quick_entry_title: "إدخال سريع للمهام",
    quick_entry_hint: "اكتب مهمة واضغط Enter لإضافتها للقائمة (أو افصل بـ /)، ثم اضغط إنشاء البورد أو Ctrl+Enter",
    quick_entry_placeholder: "علاج بابا / كتابة اللائحة للمرافعة",
    quick_entry_preview: "المهام الجاهزة",
    quick_entry_create: "إنشاء البورد",
    quick_entry_cancel: "إلغاء",
    quick_entry_duplicate: "عنوان مكرر:",

    zen_badge: "تركيز",

    load_error_title: "خطأ في تحميل البيانات",
    load_error_intro: "تعذّر قراءة أو تحليل ملف data.json. لم يتم مسح ملفك تلقائياً.",
    load_error_details_label: "تفاصيل الخطأ:",
    load_error_backup_label: "تم حفظ نسخة احتياطية في:",
    load_error_backup_failed_label: "تعذّر إنشاء نسخة احتياطية:",
    load_error_copy_path: "نسخ المسار",
    load_error_copied_path: "تم النسخ",
    load_error_start_fresh: "البدء ببيانات جديدة",
    load_error_exit: "إغلاق التطبيق",

    data_io_btn: "تصدير / استيراد",
    data_io_title: "تصدير واستيراد البيانات",
    data_io_export_json: "تصدير الكل (JSON)",
    data_io_export_json_desc: "حفظ جميع البروفايلات والمهام والملاحظات والأرشيف في ملف JSON.",
    data_io_export_md: "تصدير اليوم (Markdown)",
    data_io_export_md_desc: "تصدير لوحات ومهام وملاحظات اليوم الحالي كملف Markdown.",
    data_io_import_json: "استيراد (JSON)",
    data_io_import_json_desc: "استبدال البيانات الحالية بملف JSON مع حفظ نسخة احتياطية تلقائياً أولاً.",
    data_io_confirm_title: "تأكيد الاستيراد",
    data_io_confirm_warning: "سيتم حفظ نسخة احتياطية (.bak) من بياناتك الحالية تلقائياً قبل الاستبدال.",
    data_io_confirm_btn: "تأكيد الاستيراد",
    data_io_waiting_dialog: "بانتظار نافذة اختيار الملف...",

    tip_close: "إغلاق",
    tip_maximize: "تكبير / استعادة",
    tip_minimize: "تصغير",
    tip_switch_lang: "تبديل اللغة (AR / EN)",
    tip_prev_page: "الصفحة السابقة (PageUp / Alt+→)",
    tip_next_page: "الصفحة التالية (PageDown / Alt+←)",
    tip_page_cycle: "الانتقال للصفحة التالية",
    tip_close_esc: "إغلاق [Esc]",
    tip_toggle_complete: "تبديل الإنجاز",
    tip_expand_branch: "توسيع الفروع",
    tip_collapse_branch: "طيّ الفروع",
    tip_remove_staged: "حذف من القائمة",
};

const EN_STRINGS: UiStrings = UiStrings {
    tab_home: "HOME",
    tab_index: "INDEX",
    tab_harbor: "HARBOR",
    back_btn: "← BACK [Esc]",
    page_word: "PAGE",

    status_done: "DONE",
    status_active: "ACTIVE",
    status_cancelled: "CANCELLED",
    status_total: "TOTAL",
    status_saving: "SAVING...",
    status_auto_saved: "AUTO-SAVED",
    status_save_error: "SAVE ERROR",
    toast_undone: "UNDONE (Ctrl+Z)",

    branch_placeholder: "+ Add new branch...",
    index_new_pane_placeholder: "+ New main task title...",
    index_empty_hint: "NO MAIN PANES YET — TYPE ABOVE AND PRESS ENTER",
    task_title_placeholder: "Task title...",
    untitled: "Untitled",

    note_tag: "NOTE",
    copy_btn: "COPY",
    copied_btn: "COPIED",
    note_placeholder: "Write your notes here...",

    action_archive: "ARCHIVE",
    action_delete: "DELETE",
    action_confirm: "CONFIRM?",
    action_edit: "EDIT",
    action_open: "OPEN",
    action_active_badge: "ACTIVE",
    action_restore: "RESTORE",

    ctx_active: "ACTIVE [1]",
    ctx_completed: "COMPLETED [2]",
    ctx_cancelled: "CANCELLED [3]",
    ctx_notes: "NOTES",
    ctx_edit: "EDIT",
    ctx_add_branch: "ADD BRANCH",
    ctx_delete_task: "DELETE TASK",

    profiles_title: "DAILY PROFILES",
    profiles_tag: "PROFILES",
    profiles_new: "+ NEW PROFILE",
    profiles_carry_over: "CARRY OVER ACTIVE",
    profiles_title_placeholder: "Profile title...",
    profiles_invalid_date: "Invalid date (use YYYY-MM-DD)",

    guide_title: "QUICK START GUIDE",
    guide_bullets: [
        "1. Add Branches: Type in the bottom bar of any pane and press Enter.",
        "2. Fold Branches: Click the small [−] or [+N] anchor on a card border to collapse or expand its sub-branches.",
        "3. Notes: Single-click any card to open its paper note drawer.",
        "4. Organize Pages: Open the INDEX tab and select any task to reorder it across pages with ↑ ↓.",
    ],
    guide_cta: "START WORKING",

    harbor_title: "HARBOR",
    harbor_empty: "NO ARCHIVED PANES YET.",
    harbor_search_placeholder: "Search archived panes, subtasks, or notes...",
    harbor_no_matches: "NO MATCHING ARCHIVED PANES.",

    empty_board_title: "NO TASKS YET",
    empty_board_add: "+ ADD TASKS",

    quick_entry_title: "QUICK TASK ENTRY",
    quick_entry_hint: "PRESS ENTER TO STAGE TASKS (OR SEPARATE WITH /), THEN CLICK CREATE BOARD OR CTRL+ENTER",
    quick_entry_placeholder: "Quarterly review / Prepare client brief",
    quick_entry_preview: "STAGED TASKS",
    quick_entry_create: "CREATE BOARD",
    quick_entry_cancel: "CANCEL",
    quick_entry_duplicate: "Duplicate title:",

    zen_badge: "ZEN",

    load_error_title: "DATA LOAD ERROR",
    load_error_intro: "Failed to read or parse data.json. Your file was NOT silently overwritten.",
    load_error_details_label: "ERROR DETAILS:",
    load_error_backup_label: "BACKUP COPY SAVED AT:",
    load_error_backup_failed_label: "BACKUP FAILED:",
    load_error_copy_path: "COPY PATH",
    load_error_copied_path: "COPIED",
    load_error_start_fresh: "START FRESH",
    load_error_exit: "EXIT APP",

    data_io_btn: "EXPORT / IMPORT",
    data_io_title: "EXPORT & IMPORT DATA",
    data_io_export_json: "EXPORT ALL (JSON)",
    data_io_export_json_desc: "Save all daily profiles, tasks, notes, and archived panes to a JSON file.",
    data_io_export_md: "EXPORT TODAY (MARKDOWN)",
    data_io_export_md_desc: "Export the current day's panes, subtasks, and notes as a Markdown file.",
    data_io_import_json: "IMPORT (JSON)",
    data_io_import_json_desc: "Load a JSON backup after automatically saving a .bak copy of your current data.",
    data_io_confirm_title: "CONFIRM IMPORT",
    data_io_confirm_warning: "A timestamped .bak backup of your current data will be saved automatically before replacing.",
    data_io_confirm_btn: "CONFIRM IMPORT",
    data_io_waiting_dialog: "Waiting for file dialog...",

    tip_close: "CLOSE",
    tip_maximize: "MAXIMIZE",
    tip_minimize: "MINIMIZE",
    tip_switch_lang: "SWITCH LANGUAGE (AR / EN)",
    tip_prev_page: "Previous page (PageUp / Ctrl+Left / Alt+Left)",
    tip_next_page: "Next page (PageDown / Ctrl+Right / Alt+Right)",
    tip_page_cycle: "Cycle page",
    tip_close_esc: "Close [Esc]",
    tip_toggle_complete: "Toggle completed",
    tip_expand_branch: "Expand branches",
    tip_collapse_branch: "Collapse branches",
    tip_remove_staged: "Remove from list",
};

/// Returns the static UI localization table for `lang`.
pub fn tr(lang: AppLang) -> UiStrings {
    match lang {
        AppLang::Ar => AR_STRINGS,
        AppLang::En => EN_STRINGS,
    }
}

/// Shapes text for display when Arabic characters are present, otherwise passes
/// English/Latin strings through untouched.
pub fn disp(text: &str) -> String {
    if crate::rtl_text::contains_arabic(text) {
        shape_arabic(text)
    } else {
        text.to_string()
    }
}

/// Formats the carry-over toast message in the active UI language.
pub fn format_carry_over_toast(lang: AppLang, count: usize) -> String {
    match lang {
        AppLang::Ar => format!("تم ترحيل {} لوحات نشطة", count),
        AppLang::En => format!("Carried over {} active pane(s)", count),
    }
}
