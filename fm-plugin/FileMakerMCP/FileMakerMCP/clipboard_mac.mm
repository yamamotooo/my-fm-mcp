#import <AppKit/AppKit.h>
#include "clipboard_mac.h"

int SetFileMakerClipboard( const char* utf8Xml, int length )
{
    @autoreleasepool
    {
        NSPasteboard* pb = [NSPasteboard generalPasteboard];
        [pb clearContents];

        NSData* data = [NSData dataWithBytes:utf8Xml length:(NSUInteger)length];

        // dyn.ah62d4rv4gk8zuxnqgk = FileMaker Layout Object (.fmp12)
        BOOL ok = [pb setData:data forType:@"dyn.ah62d4rv4gk8zuxnqgk"];
        return ok ? 1 : 0;
    }
}
