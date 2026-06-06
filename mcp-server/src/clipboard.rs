/// XML の内容から FileMaker クリップボード形式を判別する
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn detect_format(xml: &str) -> (&'static str, &'static str) {
    // 戻り値: (macOS UTI, Windows format name)

    // fmxmlsnippet 形式（FM12+）: ルート開始タグの type 属性だけを見る
    // 子要素の type="FieldObj" 等と誤マッチしないようタグ末尾 '>' までに絞る
    if let Some(start) = xml.find("<fmxmlsnippet") {
        let tag_end = xml[start..].find('>').map(|i| start + i).unwrap_or(xml.len());
        let tag = &xml[start..tag_end];
        return if tag.contains(r#"type="LayoutObjectList""#) {
            ("dyn.ah62d4rv4gk8zuxnqgk", "Mac-XML2")   // レイアウトオブジェクト
        } else if tag.contains(r#"type="ScriptSteps""#) {
            ("dyn.ah62d4rv4gk8zuxnxnq", "Mac-XMSS")   // スクリプトステップ
        } else if tag.contains(r#"type="FieldObj""#) {
            ("dyn.ah62d4rv4gk8zuxngku", "Mac-XMFD")   // フィールド定義
        } else if tag.contains(r#"type="ValueListObj""#) {
            ("dyn.ah62d4rv4gk8zuxn0mu", "Mac-XMVL")   // 値一覧
        } else if tag.contains(r#"type="BaseTableObj""#) {
            ("dyn.ah62d4rv4gk8zuxnykk", "Mac-XMTB")   // テーブル定義
        } else {
            ("dyn.ah62d4rv4gk8zuxnqgk", "Mac-XML2")   // 不明 → レイアウトにフォールバック
        };
    }

    // FMObjectTransfer 形式（FM11 以前 / 旧形式）
    if xml.contains("<FMObjectTransfer") {
        ("dyn.ah62d4rv4gk8zuxnqgk", "Mac-XML2")
    } else if xml.contains("<Step ") || xml.contains("<Step\n") || xml.contains("<Step\r") {
        ("dyn.ah62d4rv4gk8zuxnxnq", "Mac-XMSS")
    } else if xml.contains("<StepList") {
        ("dyn.ah62d4rv4gk8zuxnxnq", "Mac-XMSS")
    } else if xml.contains("<Field ") || xml.contains("<FieldList") {
        ("dyn.ah62d4rv4gk8zuxngku", "Mac-XMFD")
    } else if xml.contains("<ValueList") {
        ("dyn.ah62d4rv4gk8zuxn0mu", "Mac-XMVL")
    } else if xml.contains("<BaseTable") || xml.contains("<TableList") {
        ("dyn.ah62d4rv4gk8zuxnykk", "Mac-XMTB")
    } else {
        ("dyn.ah62d4rv4gk8zuxnqgk", "Mac-XML2")   // デフォルト: レイアウト
    }
}

/// macOS: NSPasteboard へ FileMaker XML を書き込む
#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
pub fn set_layout_xml(xml: &str) -> Result<(), String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CString;
    use std::time::Instant;

    let (uti, _) = detect_format(xml);
    eprintln!("[clipboard] start: UTI={uti}, xml_len={}", xml.len());
    let uti_cstr = CString::new(uti).map_err(|e| e.to_string())?;
    let utf8_cstr = CString::new("public.utf8-plain-text").map_err(|e| e.to_string())?;
    let bytes = xml.as_bytes();

    unsafe {
        let t = Instant::now();
        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        eprintln!("[clipboard] generalPasteboard={:?}", t.elapsed());

        let t = Instant::now();
        let ns_uti: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: uti_cstr.as_ptr()];
        let ns_utf8_uti: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: utf8_cstr.as_ptr()];
        let ns_data: *mut Object = msg_send![
            class!(NSData),
            dataWithBytes: bytes.as_ptr() as *const std::ffi::c_void
            length: bytes.len()
        ];
        eprintln!("[clipboard] NSString/NSData alloc={:?}", t.elapsed());

        let t = Instant::now();
        let item: *mut Object = msg_send![class!(NSPasteboardItem), new];
        let _: bool = msg_send![item, setData: ns_data forType: ns_uti];
        let _: bool = msg_send![item, setData: ns_data forType: ns_utf8_uti];
        eprintln!("[clipboard] NSPasteboardItem setup={:?}", t.elapsed());

        let t = Instant::now();
        let ns_items: *mut Object = msg_send![class!(NSArray), arrayWithObject: item];
        let _: () = msg_send![pb, clearContents];
        let ok: bool = msg_send![pb, writeObjects: ns_items];
        eprintln!("[clipboard] writeObjects={:?} ok={ok}", t.elapsed());

        if ok {
            Ok(())
        } else {
            Err("NSPasteboard writeObjects: failed".to_string())
        }
    }
}

/// Windows: RegisterClipboardFormat で FileMaker XML を書き込む
/// 先頭 4 バイトはヘッダ（ゼロ埋め）。GetClipboardData 側が先頭 4 バイトをスキップして読む。
#[cfg(target_os = "windows")]
pub fn set_layout_xml(xml: &str) -> Result<(), String> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatA, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    let (_, win_fmt) = detect_format(xml);
    let xml_bytes = xml.as_bytes();
    let data_size = 4 + xml_bytes.len();

    let fmt_cstr = std::ffi::CString::new(win_fmt).map_err(|e| e.to_string())?;

    unsafe {
        let fmt = RegisterClipboardFormatA(fmt_cstr.as_ptr() as *const u8);
        if fmt == 0 {
            return Err("RegisterClipboardFormat failed".to_string());
        }

        let hmem = GlobalAlloc(GMEM_MOVEABLE, data_size);
        if hmem == 0 {
            return Err("GlobalAlloc failed".to_string());
        }

        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err("GlobalLock failed".to_string());
        }
        std::ptr::write_bytes(ptr, 0, 4);
        std::ptr::copy_nonoverlapping(xml_bytes.as_ptr(), ptr.add(4), xml_bytes.len());
        GlobalUnlock(hmem);

        if OpenClipboard(0) == 0 {
            return Err("OpenClipboard failed".to_string());
        }
        EmptyClipboard();
        SetClipboardData(fmt, hmem as HANDLE);
        CloseClipboard();
    }

    Ok(())
}

/// Linux / その他: FileMaker は非対応
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn set_layout_xml(_xml: &str) -> Result<(), String> {
    Err("clipboard is not supported on this platform".to_string())
}
