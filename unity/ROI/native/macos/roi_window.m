// ROI window control for macOS — the two things Unity does not expose: a window that floats above
// other apps, and one without a title bar.
//
// WHY THIS EXISTS AT ALL. The companion widget is meant to sit in a corner of the desktop while the
// player works, which is what it is FOR; a picture-in-picture inside a full-size game window is a
// different, much less useful thing. Unity has no always-on-top API on any platform, and unlike
// Windows — where `user32.dll` is a system library a P/Invoke can call directly — AppKit cannot be
// reached from C#. So macOS needs a bundle, and this is the smallest one that does the job.
//
// WE COMPILE IT OURSELVES (tools/build_mac_plugin.sh) rather than vendoring someone's binary. A
// prebuilt third-party bundle would have to be re-signed with our Team ID before notarisation, and
// a step like that is missed once and then discovered at submission. Built from this file, it is
// signed with the app in the ordinary way.
//
// EVERY CALL IS TOTAL. A missing window, a call before the window exists, a nil app — each answers
// 0 and changes nothing. The client is allowed to ask at any time; that is what makes the C# side
// able to call it without knowing the player's startup order.
//
// ON THE MAIN THREAD, ALWAYS — but never by dispatching to it from itself. Unity calls a plugin on
// the main thread, and `dispatch_sync(dispatch_get_main_queue(), ...)` from the main thread is a
// deadlock against yourself; the first build of this file did exactly that and the player died
// inside the first call. `run_on_main` runs the block inline when it is already there.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

/// Run a block on the main thread, inline when we are already on it. See the header.
static void run_on_main(dispatch_block_t block) {
    if ([NSThread isMainThread]) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}

/// The player's window. `mainWindow` is nil while the app is inactive — which is exactly when a
/// floating widget matters — so fall back to the first visible one.
static NSWindow *roi_window(void) {
    NSApplication *app = NSApp;
    if (app == nil) return nil;
    NSWindow *w = [app mainWindow];
    if (w != nil) return w;
    for (NSWindow *candidate in [app windows]) {
        if ([candidate isVisible]) return candidate;
    }
    return nil;
}

/// Float above other applications, or return to the normal layer.
///
/// `NSFloatingWindowLevel` (3), not a higher one: the levels above it belong to the system's own
/// furniture — the menu bar, the Dock, screen savers — and a game that installs itself over those
/// is a game the player cannot escape. Floating is "above other apps' windows", which is the ask.
int roi_window_set_topmost(int on) {
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil) return;
        [w setLevel:(on ? NSFloatingWindowLevel : NSNormalWindowLevel)];
        // So it stays visible when the player switches to another Space or a fullscreen app —
        // otherwise "always on top" quietly means "on top of this desktop only".
        if (on) {
            [w setCollectionBehavior:(NSWindowCollectionBehaviorCanJoinAllSpaces
                                      | NSWindowCollectionBehaviorFullScreenAuxiliary)];
        } else {
            [w setCollectionBehavior:NSWindowCollectionBehaviorDefault];
        }
        ok = 1;
    });
    return ok;
}

// Unity requests the resize after the C# frame. Do not change the window's runtime class:
// AppKit uses KVO on it, and swapping isa breaks titlebar observer teardown. Forward every other
// window to its original timing; only the current Companion window gets zero-duration resizing.
static __weak NSWindow *immediateWindow;
static Class resizeHookClass;
static IMP originalResizeTime;
static const char *resizeTimeEncoding;
static NSWindowAnimationBehavior previousAnimationBehavior;
static NSTimeInterval immediate_resize_time(id window, SEL selector, NSRect frame) {
    if (window == immediateWindow) return 0;
    return ((NSTimeInterval (*)(id, SEL, NSRect))originalResizeTime)(window, selector, frame);
}
static void set_immediate_resize(NSWindow *w, BOOL on) {
    if (on && immediateWindow == nil) {
        resizeHookClass = [w class]; // Stable base class even if KVO installs a notifying subclass.
        Method method = class_getInstanceMethod(resizeHookClass, @selector(animationResizeTime:));
        originalResizeTime = method_getImplementation(method);
        resizeTimeEncoding = method_getTypeEncoding(method);
        previousAnimationBehavior = w.animationBehavior;
        immediateWindow = w;
        class_replaceMethod(resizeHookClass, @selector(animationResizeTime:), (IMP)immediate_resize_time, resizeTimeEncoding);
        w.animationBehavior = NSWindowAnimationBehaviorNone;
    } else if (!on && immediateWindow != nil) {
        class_replaceMethod(resizeHookClass, @selector(animationResizeTime:), originalResizeTime, resizeTimeEncoding);
        immediateWindow.animationBehavior = previousAnimationBehavior;
        immediateWindow = nil;
        resizeHookClass = Nil;
    }
}

/// Drop the title bar, or put it back. Answers 0 when it REFUSED, which is a real answer the
/// caller acts on rather than a failure to report.
///
/// TWO THINGS THIS HAS TO GET RIGHT, and the first cost a crash:
///
///   NEVER TOUCH THE FULLSCREEN BIT. `setStyleMask:` replaces the whole mask, and clearing
///   `NSWindowStyleMaskFullScreen` outside a real fullscreen transition raises an uncaught
///   NSGenericException — which in a Unity player is not an error to catch, it is the process
///   ending. So a fullscreen window is refused outright, and the bit is carried across otherwise.
///
///   REPLACE, DO NOT OR. A "borderless" window that keeps `titled` still draws its bar, so the
///   decoration bits are set wholesale rather than added to.
int roi_window_set_borderless(int on) {
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil) return;
        NSWindowStyleMask mask = [w styleMask];
        if (mask & NSWindowStyleMaskFullScreen) return;   // refused — see above
        set_immediate_resize(w, on);
        if (on) {
            [w setStyleMask:NSWindowStyleMaskBorderless];
            // A borderless window is not key by default, and one that cannot take focus cannot be
            // clicked — which would make the widget's own right-click do nothing.
            [w makeKeyAndOrderFront:nil];
        } else {
            [w setStyleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable
                             | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable)];
        }
        ok = 1;
    });
    return ok;
}

/// Is the window in macOS fullscreen? The caller waits for this to be false before asking for a
/// style change — Unity applies a resolution change at the end of a frame, so "I asked for
/// windowed" and "the window is windowed" are not the same moment.
int roi_window_is_fullscreen(void) {
    __block int fs = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w != nil) fs = ([w styleMask] & NSWindowStyleMaskFullScreen) ? 1 : 0;
    });
    return fs;
}

/// Is the window currently floating? Lets the C# side restore what it found rather than assume the
/// normal level — the same reason the Godot widget saves `_saved_on_top`.
int roi_window_get_topmost(void) {
    __block int on = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w != nil) on = ([w level] >= NSFloatingWindowLevel) ? 1 : 0;
    });
    return on;
}

// Alpha must survive both Unity's framebuffer and AppKit's window/layer composition.
// Keep the original window/layer state so leaving Companion restores ordinary Battle.
static NSWindow *alphaWindow;
static NSColor *originalBackground;
static BOOL originalOpaque, originalShadow;
static NSMapTable<CALayer *, NSNumber *> *originalLayerOpacity;

static void transparent_layer(CALayer *layer) {
    if (layer == nil) return;
    if ([originalLayerOpacity objectForKey:layer] == nil)
        [originalLayerOpacity setObject:@(layer.opaque) forKey:layer];
    layer.opaque = NO;
    for (CALayer *child in layer.sublayers) transparent_layer(child);
}
static void transparent_view(NSView *view) {
    transparent_layer(view.layer);
    for (NSView *child in view.subviews) transparent_view(child);
}
int roi_window_set_transparent(int on) {
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil || (w.styleMask & NSWindowStyleMaskFullScreen)) return;
        if (on) {
            if (alphaWindow == nil) {
                alphaWindow = w;
                originalBackground = w.backgroundColor;
                originalOpaque = w.opaque;
                originalShadow = w.hasShadow;
                // Unity replaces Metal layers on resize. Retaining removed layers also keeps
                // their drawable resources alive; restore only layers still owned by the view.
                originalLayerOpacity = [NSMapTable weakToStrongObjectsMapTable];
            }
            w.opaque = NO;
            w.backgroundColor = NSColor.clearColor;
            w.hasShadow = NO; // No rectangular shadow around empty transparent gaps.
            transparent_view(w.contentView);
        } else if (alphaWindow != nil) {
            alphaWindow.opaque = originalOpaque;
            alphaWindow.backgroundColor = originalBackground;
            alphaWindow.hasShadow = originalShadow;
            for (CALayer *layer in originalLayerOpacity)
                layer.opaque = [[originalLayerOpacity objectForKey:layer] boolValue];
            originalLayerOpacity = nil;
            originalBackground = nil;
            alphaWindow = nil;
        }
        ok = 1;
    });
    return ok;
}
int roi_window_get_transparent(void) {
    __block int on = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w != nil) on = !w.opaque && w.backgroundColor.alphaComponent == 0
            && (w.contentView.layer == nil || !w.contentView.layer.opaque);
    });
    return on;
}

// Synchronous movement: Unity's MoveMainWindowTo returns an AsyncOperation whose finalizer
// crashed during Mono shutdown after repeated companion resizes. Keep capture, placement and
// restore in AppKit desktop points so Retina pixels and native window coordinates cannot mix.
int roi_window_get_position(int *x, int *y) {
    if (x == NULL || y == NULL) return 0;
    *x = 0; *y = 0;
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil) return;
        *x = (int)NSMinX(w.frame); *y = (int)NSMaxY(w.frame);
        ok = 1;
    });
    return ok;
}
int roi_window_set_position(int x, int y) {
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil || (w.styleMask & NSWindowStyleMaskFullScreen)) return;
        [w setFrameTopLeftPoint:NSMakePoint(x, y)];
        ok = 1;
    });
    return ok;
}
int roi_window_place_in_corner(void) {
    __block int ok = 0;
    run_on_main(^{
        NSWindow *w = roi_window();
        if (w == nil || w.screen == nil || (w.styleMask & NSWindowStyleMaskFullScreen)) return;
        NSRect area = w.screen.visibleFrame;
        [w setFrameOrigin:NSMakePoint(MAX(NSMinX(area), NSMaxX(area) - w.frame.size.width - 12),
                                     NSMinY(area) + 12)];
        ok = 1;
    });
    return ok;
}
