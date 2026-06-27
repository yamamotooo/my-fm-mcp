/// FileMaker のヘルプメニュー検索を使い、機能の場所をメニュー上でハイライトする
pub fn focus_help_search(keyword: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return focus_help_search_macos(keyword);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = keyword;
        Err("この機能は macOS のみサポートされています".to_string())
    }
}

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
fn focus_help_search_macos(keyword: &str) -> Result<String, String> {
    use accessibility_sys::*;
    use core_foundation::array::{kCFTypeArrayCallBacks, CFArrayCreate};
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::CFString;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let pid = find_filemaker_pid()
        .ok_or_else(|| "FileMaker Pro が起動していません".to_string())?;

    unsafe {
        let app = AXUIElementCreateApplication(pid);

        // メニューバー取得（失敗 = アクセシビリティ権限なし）
        let mb_cf = ax_copy_attr(app, "AXMenuBar").ok_or_else(|| {
            "メニューバー取得失敗。システム設定 > プライバシーとセキュリティ > アクセシビリティ で許可してください".to_string()
        })?;
        let menu_bar = mb_cf.as_CFTypeRef() as AXUIElementRef;

        // ヘルプメニューをタイトルで探す（インデックス固定より確実）
        let menu_children = ax_children_raw(menu_bar);
        let help_item = menu_children
            .iter()
            .find(|&&item| {
                ax_get_string(item, "AXTitle")
                    .map(|t| t == "ヘルプ" || t == "Help")
                    .unwrap_or(false)
            })
            .copied()
            .ok_or_else(|| "ヘルプメニューが見つかりません".to_string())?;

        // FileMaker をフォアグラウンドへ
        let ns_app: *mut Object = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        let _: bool = msg_send![ns_app, activateWithOptions: 1u64];
        std::thread::sleep(std::time::Duration::from_millis(300));

        // ヘルプメニューを開く
        let press = CFString::new("AXPress");
        AXUIElementPerformAction(help_item, press.as_concrete_TypeRef());

        // 検索フィールドにフォーカスが当たるまで待つ（最大 1 秒）
        let search_field_cf = {
            let mut found = None;
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Some(cf) = ax_copy_attr(app, "AXFocusedUIElement") {
                    let elem = cf.as_CFTypeRef() as AXUIElementRef;
                    if ax_get_string(elem, "AXSubrole").as_deref() == Some("AXSearchField") {
                        found = Some(cf);
                        break;
                    }
                }
            }
            found
        };

        let sf_cf = search_field_cf
            .ok_or_else(|| "ヘルプ検索フィールドが見つかりません".to_string())?;
        let search_field = sf_cf.as_CFTypeRef() as AXUIElementRef;

        // キーワードを入力
        let attr_name = CFString::new("AXValue");
        let kw_cf = CFString::new(keyword);
        AXUIElementSetAttributeValue(search_field, attr_name.as_concrete_TypeRef(), kw_cf.as_CFTypeRef());

        // 検索結果が出るまで待つ
        std::thread::sleep(std::time::Duration::from_millis(600));

        // 検索フィールドの兄弟テーブルから最初の行を選択（メニュー上でハイライト）
        let mut selected_title: Option<String> = None;
        if let Some(parent_cf) = ax_copy_attr(search_field, "AXParent") {
            let parent = parent_cf.as_CFTypeRef() as AXUIElementRef;
            let siblings = ax_children_raw(parent);
            if siblings.len() >= 2 {
                let table = siblings[1];
                let rows = ax_children_raw(table);
                if !rows.is_empty() {
                    selected_title = ax_get_string(rows[0], "AXTitle");
                    let row_ref = rows[0] as CFTypeRef;
                    let sel_attr = CFString::new("AXSelectedRows");
                    let arr = CFArrayCreate(std::ptr::null(), &row_ref, 1, &kCFTypeArrayCallBacks);
                    AXUIElementSetAttributeValue(table, sel_attr.as_concrete_TypeRef(), arr as CFTypeRef);
                    CFRelease(arr as CFTypeRef);
                }
            }
        }

        CFRelease(app as CFTypeRef);

        match selected_title {
            Some(title) => Ok(format!(
                "「{}」でヘルプ検索し、「{}」をハイライトしました。FileMaker のメニューに移動先が表示されます。",
                keyword, title
            )),
            None => Ok(format!(
                "「{}」でヘルプ検索しました（一致する結果がありませんでした）。",
                keyword
            )),
        }
    }
}

#[cfg(target_os = "macos")]
fn find_filemaker_pid() -> Option<i32> {
    let output = std::process::Command::new("ps")
        .args(["-ax", "-o", "pid=,command="])
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    for line in stdout.lines() {
        if line.contains("FileMaker Pro.app") {
            let pid: i32 = line.trim().splitn(2, ' ').next()?.trim().parse().ok()?;
            return Some(pid);
        }
    }
    None
}

#[cfg(target_os = "macos")]
unsafe fn ax_copy_attr(
    element: accessibility_sys::AXUIElementRef,
    attr: &str,
) -> Option<core_foundation::base::CFType> {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    let key = CFString::new(attr);
    let mut value: CFTypeRef = std::ptr::null();
    let err = accessibility_sys::AXUIElementCopyAttributeValue(
        element,
        key.as_concrete_TypeRef(),
        &mut value,
    );
    if err == accessibility_sys::kAXErrorSuccess && !value.is_null() {
        Some(CFType::wrap_under_create_rule(value))
    } else {
        None
    }
}

/// AXChildren を raw ポインタのベクタとして返す。配列は意図的にリーク（FM 実行中は有効）。
#[cfg(target_os = "macos")]
unsafe fn ax_children_raw(
    element: accessibility_sys::AXUIElementRef,
) -> Vec<accessibility_sys::AXUIElementRef> {
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation::base::{CFTypeRef, TCFType};
    use core_foundation::string::CFString;

    let key = CFString::new("AXChildren");
    let mut value: CFTypeRef = std::ptr::null();
    let err = accessibility_sys::AXUIElementCopyAttributeValue(
        element,
        key.as_concrete_TypeRef(),
        &mut value,
    );
    if err != accessibility_sys::kAXErrorSuccess || value.is_null() {
        return vec![];
    }
    let arr = value as CFArrayRef;
    let count = CFArrayGetCount(arr);
    (0..count)
        .map(|i| CFArrayGetValueAtIndex(arr, i) as accessibility_sys::AXUIElementRef)
        .collect()
    // arr は意図的にリーク（本関数はワンショット操作のみで使用）
}

#[cfg(target_os = "macos")]
unsafe fn ax_get_string(
    element: accessibility_sys::AXUIElementRef,
    attr: &str,
) -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};

    let cf = ax_copy_attr(element, attr)?;
    Some(CFString::wrap_under_get_rule(cf.as_CFTypeRef() as CFStringRef).to_string())
}
