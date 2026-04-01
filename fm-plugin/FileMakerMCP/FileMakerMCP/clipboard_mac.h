#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Write XML data to the macOS pasteboard as a FileMaker layout object.
// UTI: dyn.ah62d4rv4gk8zuxnqgk  (Layout Object, .fmp12)
// Returns 1 on success, 0 on failure.
int SetFileMakerClipboard( const char* utf8Xml, int length );

#ifdef __cplusplus
}
#endif
