/// ヘルプメニューで keyword を検索し menu_item に一致する行をハイライトする。
pub fn navigate_to_feature(keyword: &str, menu_item: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return focus_help_search_macos(keyword, menu_item);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (keyword, menu_item);
        Err("この機能は macOS のみサポートされています".to_string())
    }
}

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGPoint { x: f64, y: f64 }

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSize { width: f64, height: f64 }

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *const std::ffi::c_void,
        keycode: u16,
        keydown: bool,
    ) -> *mut std::ffi::c_void;
    fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
}

/// キーコードを HID キューへ送信する
#[cfg(target_os = "macos")]
unsafe fn post_key_event(keycode: u16) {
    use core_foundation::base::CFTypeRef;
    let down = CGEventCreateKeyboardEvent(std::ptr::null(), keycode, true);
    CGEventPost(0, down);
    core_foundation::base::CFRelease(down as CFTypeRef);
    let up = CGEventCreateKeyboardEvent(std::ptr::null(), keycode, false);
    CGEventPost(0, up);
    core_foundation::base::CFRelease(up as CFTypeRef);
}

#[cfg(target_os = "macos")]
fn focus_help_search_macos(keyword: &str, menu_item: &str) -> Result<String, String> {
    use accessibility_sys::*;
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

        // 検索フィールドの兄弟テーブルから対象行を選択（メニュー上でハイライト）
        let mut selected_title: Option<String> = None;
        let mut all_candidates: Vec<String> = Vec::new();

        if let Some(parent_cf) = ax_copy_attr(search_field, "AXParent") {
            let parent = parent_cf.as_CFTypeRef() as AXUIElementRef;
            let siblings = ax_children_raw(parent);
            if siblings.len() >= 2 {
                let table = siblings[1];
                let rows = ax_children_raw(table);

                // 各行のテキストを収集（行自体 → 子 → 孫の順に試す）
                let row_texts: Vec<String> = rows
                    .iter()
                    .copied()
                    .filter_map(|row| ax_row_text(row))
                    .collect();

                all_candidates = row_texts.clone();

                // menu_item が空なら先頭行、それ以外は部分一致で探す
                let target_idx = if menu_item.is_empty() {
                    if row_texts.is_empty() { None } else { Some(0) }
                } else {
                    row_texts.iter().position(|t| t.contains(menu_item))
                };

                if let Some(idx) = target_idx {
                    selected_title = row_texts.get(idx).cloned();
                    // (1 + idx) 回 Arrow Down: 1 回目で row 0 を通過し、
                    // 続けて idx 回で目的行へ移動してハイライト。
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    for _ in 0..=idx {
                        post_key_event(125);
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                }
            }
        }

        CFRelease(app as CFTypeRef);

        let candidates_str = if all_candidates.is_empty() {
            String::new()
        } else {
            format!("\n候補: {}", all_candidates.join(" / "))
        };

        match selected_title {
            Some(title) => Ok(format!(
                "「{}」で検索し「{}」をハイライトしました。{}",
                keyword, title, candidates_str
            )),
            None => Ok(format!(
                "「{}」で検索しましたが一致する項目がありませんでした。{}",
                keyword, candidates_str
            )),
        }
    }
}

/// 行のテキストを AXTitle / AXValue → 子要素 → 孫要素の順に試みて取得する
#[cfg(target_os = "macos")]
unsafe fn ax_row_text(row: accessibility_sys::AXUIElementRef) -> Option<String> {
    // 行自体の属性
    for attr in &["AXTitle", "AXValue"] {
        if let Some(t) = ax_get_string(row, attr) {
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    // 子要素（AXCell など）
    for child in ax_children_raw(row) {
        for attr in &["AXTitle", "AXValue"] {
            if let Some(t) = ax_get_string(child, attr) {
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
        // 孫要素（AXStaticText など）
        for grandchild in ax_children_raw(child) {
            for attr in &["AXTitle", "AXValue"] {
                if let Some(t) = ax_get_string(grandchild, attr) {
                    if !t.is_empty() {
                        return Some(t);
                    }
                }
            }
        }
    }
    None
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

    // AXValue は CFNumber / CFBoolean を返すこともある。
    // 型チェックなしに CFString として扱うと CFStringGetLength が ObjC 例外を投げる。
    if !cf_is_string(cf.as_CFTypeRef()) {
        return None;
    }

    Some(CFString::wrap_under_get_rule(cf.as_CFTypeRef() as CFStringRef).to_string())
}

/// CFTypeRef が CFString かどうかを型 ID で確認する。
#[cfg(target_os = "macos")]
fn cf_is_string(cf: core_foundation::base::CFTypeRef) -> bool {
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFGetTypeID(cf: core_foundation::base::CFTypeRef) -> core_foundation::base::CFTypeID;
        fn CFStringGetTypeID() -> core_foundation::base::CFTypeID;
    }
    unsafe { !cf.is_null() && CFGetTypeID(cf) == CFStringGetTypeID() }
}

// ─────────────────────────────────────────────────────────────────────────────
// await_for_user_interaction
// ─────────────────────────────────────────────────────────────────────────────

/// FileMaker のダイアログ出現を待機する。
/// dialog_title に部分一致するウィンドウが現れるまで polling する。
pub fn await_for_user_interaction(dialog_title: &str, timeout_sec: u64) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return await_for_user_interaction_macos(dialog_title, timeout_sec);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (dialog_title, timeout_sec);
        Err("この機能は macOS のみサポートされています".to_string())
    }
}

#[cfg(target_os = "macos")]
fn await_for_user_interaction_macos(dialog_title: &str, timeout_sec: u64) -> Result<String, String> {
    use accessibility_sys::*;
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};

    let pid = find_filemaker_pid()
        .ok_or_else(|| "FileMaker Pro が起動していません".to_string())?;

    let iterations = (timeout_sec * 10).max(1); // 100ms × n

    unsafe {
        let app = AXUIElementCreateApplication(pid);

        for _ in 0..iterations {
            std::thread::sleep(std::time::Duration::from_millis(100));

            if let Some(wins_cf) = ax_copy_attr(app, "AXWindows") {
                use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
                let arr = wins_cf.as_CFTypeRef() as CFArrayRef;
                let count = CFArrayGetCount(arr);
                for i in 0..count {
                    let win = CFArrayGetValueAtIndex(arr, i) as AXUIElementRef;
                    if ax_get_string(win, "AXSubrole").as_deref() == Some("AXDialog") {
                        let title = ax_get_string(win, "AXTitle").unwrap_or_default();
                        if dialog_title.is_empty() || title.contains(dialog_title) {
                            CFRelease(app as CFTypeRef);
                            return Ok(format!("ダイアログを検出しました: {title}"));
                        }
                    }
                }
            }
        }

        CFRelease(app as CFTypeRef);
    }

    Err(format!("タイムアウト({timeout_sec}秒): ダイアログが検出されませんでした"))
}

// ─────────────────────────────────────────────────────────────────────────────
// highlight_to_feature
// ─────────────────────────────────────────────────────────────────────────────

/// Accessibility API で要素座標を取得し、周囲を透明ウィンドウで 3 秒間強調表示する。
pub fn highlight_to_feature(tab_name: Option<&str>, element_name: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return highlight_to_feature_macos(tab_name, element_name);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (tab_name, element_name);
        Err("この機能は macOS のみサポートされています".to_string())
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueGetValue(
        value: *const std::ffi::c_void,
        the_type: u32,
        value_ptr: *mut std::ffi::c_void,
    ) -> u8;
}

#[cfg(target_os = "macos")]
fn highlight_to_feature_macos(tab_name: Option<&str>, element_name: &str) -> Result<String, String> {
    use accessibility_sys::*;
    use core_foundation::base::{CFRelease, CFTypeRef};

    let pid = find_filemaker_pid()
        .ok_or_else(|| "FileMaker Pro が起動していません".to_string())?;

    unsafe {
        let app = AXUIElementCreateApplication(pid);

        // ダイアログウィンドウを探す
        let dialog = ax_find_dialog(app, "")
            .ok_or_else(|| "ダイアログが見つかりません".to_string())?;

        // タブ切り替え
        if let Some(tab) = tab_name {
            ax_switch_tab(dialog, tab)?;
            std::thread::sleep(std::time::Duration::from_millis(300));
        }

        // BFS で要素を探す
        let element = ax_bfs_find_element(dialog, element_name)
            .ok_or_else(|| format!("要素が見つかりません: {element_name}"))?;

        // 座標取得
        let (x, y, w, h) = ax_get_element_frame(element)?;

        // 赤枠オーバーレイを 3 秒表示
        ax_show_highlight_overlay(x, y, w, h, element_name);

        // ax_find_dialog で CFRetain した分を解放
        use core_foundation::base::CFRelease as CFReleaseRaw;
        CFReleaseRaw(dialog as CFTypeRef);
        CFRelease(app as CFTypeRef);

        Ok(format!("「{element_name}」を強調表示しました (x:{x:.0} y:{y:.0} w:{w:.0} h:{h:.0})"))
    }
}

/// AXWindows から AXSubrole==AXDialog のウィンドウを返す。
/// title_contains が空なら最初のダイアログを返す。
#[cfg(target_os = "macos")]
/// 戻り値は CFRetain 済み。呼び出し元が CFRelease する責任を持つ。
unsafe fn ax_find_dialog(
    app: accessibility_sys::AXUIElementRef,
    title_contains: &str,
) -> Option<accessibility_sys::AXUIElementRef> {
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation::base::{CFRetain, CFTypeRef, TCFType};

    let wins_cf = ax_copy_attr(app, "AXWindows")?;
    let arr = wins_cf.as_CFTypeRef() as CFArrayRef;
    let count = CFArrayGetCount(arr);
    for i in 0..count {
        let win = CFArrayGetValueAtIndex(arr, i) as accessibility_sys::AXUIElementRef;
        if ax_get_string(win, "AXSubrole").as_deref() == Some("AXDialog") {
            let matched = title_contains.is_empty()
                || ax_get_string(win, "AXTitle")
                    .map(|t| t.contains(title_contains))
                    .unwrap_or(false);
            if matched {
                // wins_cf は関数から返った直後に drop されるので先に retain する
                CFRetain(win as CFTypeRef);
                return Some(win);
            }
        }
    }
    None
}

/// dialog 内の AXTabGroup からタブを BFS で探して AXPress する。
#[cfg(target_os = "macos")]
unsafe fn ax_switch_tab(
    dialog: accessibility_sys::AXUIElementRef,
    tab_name: &str,
) -> Result<(), String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    let mut queue = vec![dialog];
    while let Some(elem) = queue.first().cloned() {
        queue.remove(0);
        if ax_get_string(elem, "AXRole").as_deref() == Some("AXTabGroup") {
            for child in ax_children_raw(elem) {
                let title = ax_get_string(child, "AXTitle").unwrap_or_default();
                if title.contains(tab_name) {
                    let press = CFString::new("AXPress");
                    accessibility_sys::AXUIElementPerformAction(child, press.as_concrete_TypeRef());
                    return Ok(());
                }
            }
        }
        queue.extend(ax_children_raw(elem));
    }
    Err(format!("タブが見つかりません: {tab_name}"))
}

/// root 以下を BFS で走査し element_name に部分一致する要素を返す。
/// AXTitle / AXDescription / AXLabel / AXValue の順に照合する。
#[cfg(target_os = "macos")]
unsafe fn ax_bfs_find_element(
    root: accessibility_sys::AXUIElementRef,
    name: &str,
) -> Option<accessibility_sys::AXUIElementRef> {
    let mut queue = vec![root];
    let mut visited = 0usize;
    while !queue.is_empty() && visited < 5000 {
        let elem = queue.remove(0);
        visited += 1;
        for attr in &["AXTitle", "AXDescription", "AXLabel", "AXValue"] {
            if ax_get_string(elem, attr)
                .map(|t| t.contains(name))
                .unwrap_or(false)
            {
                return Some(elem);
            }
        }
        queue.extend(ax_children_raw(elem));
    }
    None
}

/// AXPosition / AXSize から (x, y, width, height) を取得する。
/// 座標系は macOS AX 標準（スクリーン左上原点、y 下向き）。
#[cfg(target_os = "macos")]
unsafe fn ax_get_element_frame(
    element: accessibility_sys::AXUIElementRef,
) -> Result<(f64, f64, f64, f64), String> {
    use core_foundation::base::TCFType;

    let pos_cf = ax_copy_attr(element, "AXPosition")
        .ok_or_else(|| "AXPosition が取得できません".to_string())?;
    let siz_cf = ax_copy_attr(element, "AXSize")
        .ok_or_else(|| "AXSize が取得できません".to_string())?;

    let mut pt = CGPoint { x: 0.0, y: 0.0 };
    let mut sz = CGSize { width: 0.0, height: 0.0 };

    // kAXValueCGPointType = 1, kAXValueCGSizeType = 2
    AXValueGetValue(pos_cf.as_CFTypeRef(), 1, &mut pt as *mut _ as *mut _);
    AXValueGetValue(siz_cf.as_CFTypeRef(), 2, &mut sz as *mut _ as *mut _);

    Ok((pt.x, pt.y, sz.width, sz.height))
}

/// NSWindow 赤ハイライト → 失敗時は osascript 通知 + スリープにフォールバック。
#[cfg(target_os = "macos")]
unsafe fn ax_show_highlight_overlay(ax_x: f64, ax_y: f64, w: f64, h: f64, element_name: &str) {
    if !try_nswindow_overlay(ax_x, ax_y, w, h) {
        // CLI / NSApp 未起動環境: osascript 通知で代替
        show_osascript_notification(element_name, ax_x, ax_y, w, h);
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}


/// objc2 crates を使って赤枠（半透明）の浮動 NSWindow を 3 秒表示する。
/// setActivationPolicy(Accessory) でウィンドウサーバーに接続し、
/// orderFrontRegardless でアクティベーション不要で表示する。
/// 成功時 true、MainThreadMarker 取得失敗や NSWindow 生成失敗時は false。
#[cfg(target_os = "macos")]
fn try_nswindow_overlay(ax_x: f64, ax_y: f64, w: f64, h: f64) -> bool {
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType,
        NSColor, NSScreen, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::MainThreadMarker;
    use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize};

    // AppKit は必ずメインスレッドで操作する
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[highlight] not on main thread");
        return false;
    };

    unsafe {
        // CLI プロセスをウィンドウサーバーへ接続（Dock アイコンなし）
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        // AX 座標 (左上原点) → NSWindow 座標 (左下原点) に変換
        let screen_h = NSScreen::mainScreen(mtm)
            .map(|s| s.frame().size.height)
            .unwrap_or(1080.0);

        let border = 3.0_f64;
        let ns_y = screen_h - ax_y - h;
        let rect = NSRect::new(
            NSPoint::new(ax_x - border, ns_y - border),
            NSSize::new(w + border * 2.0, h + border * 2.0),
        );

        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        );

        window.setOpaque(false);
        window.setIgnoresMouseEvents(true);
        // 半透明の赤ハイライト（CALayer 枠線の代わり、依存クレートを最小化）
        window.setBackgroundColor(Some(
            &NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.0, 0.0, 0.35),
        ));
        // NSModalPanelWindowLevel(8) + 1 で FileMaker ダイアログの上に浮かせる
        // NSWindowLevel は isize の型エイリアス
        window.setLevel(objc2_app_kit::NSModalPanelWindowLevel + 1);

        // orderFrontRegardless: アクティベーション不要で表示
        // makeKeyAndOrderFront は app が非アクティブ時に ObjC 例外を投げるため使わない
        window.orderFrontRegardless();

        // 3 秒間 NSRunLoop を回して描画を維持（runUntilDate が最もシンプル）
        let end_date = NSDate::dateWithTimeIntervalSinceNow(3.0);
        NSRunLoop::currentRunLoop().runUntilDate(&end_date);

        window.close();
    }

    true
}

/// osascript で通知バナーを出す（CLI / MCP サーバー共用）。
#[cfg(target_os = "macos")]
fn show_osascript_notification(element_name: &str, x: f64, y: f64, w: f64, h: f64) {
    let msg = format!(
        "「{element_name}」を検出 — x:{x:.0} y:{y:.0} w:{w:.0} h:{h:.0}"
    );
    // AppleScript 文字列内のダブルクォートをエスケープ
    let safe_msg = msg.replace('"', "\\\"");
    let script = format!(
        r#"display notification "{safe_msg}" with title "FileMaker MCP" sound name "Ping""#
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .ok();
}
