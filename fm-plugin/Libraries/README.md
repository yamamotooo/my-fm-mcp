# FileMaker Plugin SDK Libraries

The binary files in this directory are **not included** in this repository due to licensing restrictions.

## How to Set Up

1. Download the **FileMaker Plugin SDK** from the [Claris Developer page](https://www.claris.com/ja/resources/downloads/).
2. Copy the SDK library files into the following directories:

```
fm-plugin/Libraries/
├── Mac/
│   └── FMWrapper.framework/        ← Copy from SDK: Libraries/Mac/
├── Win/
│   └── x64/
│       └── FMWrapper.lib           ← Copy from SDK: Libraries/Win/x64/
├── Linux/
│   ├── U22/
│   │   ├── arm64/
│   │   │   └── libFMWrapper.so     ← Copy from SDK: Libraries/Linux/U22/arm64/
│   │   └── x64/
│   │       └── libFMWrapper.so     ← Copy from SDK: Libraries/Linux/U22/x64/
│   └── U24/
│       ├── arm64/
│       │   └── libFMWrapper.so     ← Copy from SDK: Libraries/Linux/U24/arm64/
│       └── x64/
│           └── libFMWrapper.so     ← Copy from SDK: Libraries/Linux/U24/x64/
├── iphoneos/
│   └── iOSAppSDK.framework/        ← Copy from SDK: Libraries/iphoneos/
└── iphonesimulator/
    └── iOSAppSDK.framework/        ← Copy from SDK: Libraries/iphonesimulator/
```

3. Once the libraries are in place, build the plugin:
   - **macOS**: Open `FileMakerMCP.xcodeproj` in Xcode and build the `FileMakerMCP` scheme.
   - **Windows**: Open `FileMakerMCP.sln` in Visual Studio and build in Release x64.
   - **Linux**: Run `bash linux_build_plugin.sh` in the `FileMakerMCP/` directory.
