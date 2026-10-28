#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
int roi_window_set_transparent(int on);
int main() { @autoreleasepool {
 [NSApplication sharedApplication];
 NSWindow *w=[[NSWindow alloc] initWithContentRect:NSMakeRect(80,80,320,180) styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
 [w makeKeyAndOrderFront:nil]; w.contentView.wantsLayer=YES;
 __weak CALayer *removed;
 @autoreleasepool {
  [CATransaction begin]; [CATransaction setDisableActions:YES];
  CALayer *layer=[CALayer layer]; layer.opaque=YES;
  [w.contentView.layer addSublayer:layer]; removed=layer;
  roi_window_set_transparent(1);
  [layer removeFromSuperlayer];
  [CATransaction commit]; [CATransaction flush];
 }
 @autoreleasepool { [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:.5]]; }
 BOOL released=removed==nil;
 roi_window_set_transparent(0);
 printf("replaced native layer released=%d\n",released);
 [w orderOut:nil]; return released?0:1;
} }
