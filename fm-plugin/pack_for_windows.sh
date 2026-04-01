#!/bin/bash
# fm-plugin を Windows ビルド用に zip にまとめる
# 使い方: bash pack_for_windows.sh
# 出力: fm-plugin-win.zip (このスクリプトと同じディレクトリに生成)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT="$SCRIPT_DIR/fm-plugin-win.zip"

cd "$SCRIPT_DIR/.."

rm -f "$OUTPUT"

zip -r "$OUTPUT" fm-plugin \
  -x "*.DS_Store" \
  -x "fm-plugin/FileMakerMCP/FileMakerMCP.xcodeproj/*" \
  -x "fm-plugin/Libraries/Mac/*" \
  -x "fm-plugin/Libraries/Linux/*" \
  -x "fm-plugin/Libraries/iphoneos/*" \
  -x "fm-plugin/Libraries/iphonesimulator/*" \
  -x "fm-plugin/FileMakerMCP/FileMakerMCP/Info.plist" \
  -x "fm-plugin/FileMakerMCP/linux_build_plugin.sh" \
  -x "fm-plugin/FileMakerMCP/CMakeLists.txt" \
  -x "fm-plugin/pack_for_windows.sh"

echo "Done: $OUTPUT"
