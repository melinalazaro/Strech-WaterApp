// Exercise the production placement logic with synthetic screens and a window
// double. This test does not query the cursor or create an application window.
#import "../src-tauri/native/reminder.m"
#include <assert.h>

@interface TestScreen : NSObject
@property(nonatomic) NSRect frame;
@property(nonatomic) NSRect visibleFrame;
@property(nonatomic) CGFloat backingScaleFactor;
@end
@implementation TestScreen
@end

@interface TestWindow : NSObject
@property(nonatomic) NSRect frame;
@property(nonatomic) NSUInteger moves;
- (void)setFrameOrigin:(NSPoint)origin;
@end
@implementation TestWindow
- (void)setFrameOrigin:(NSPoint)origin {
    self.frame = NSMakeRect(origin.x, origin.y, NSWidth(self.frame), NSHeight(self.frame));
    self.moves++;
}
@end

static NSScreen *screen(NSRect frame, NSRect usable, CGFloat scale) {
    TestScreen *result = [TestScreen new];
    result.frame = frame;
    result.visibleFrame = usable;
    result.backingScaleFactor = scale;
    return (NSScreen *)result;
}

static void check(TestWindow *window, NSArray<NSScreen *> *screens,
                  NSPoint cursor, NSPoint expected) {
    NSUInteger moves = window.moves;
    assert(positionWindowAtPoint((NSWindow *)window, screens, cursor, 16));
    assert(NSEqualPoints(window.frame.origin, expected));
    assert(NSEqualSizes(window.frame.size, NSMakeSize(290, 298)));
    assert(window.moves == moves + 1);
}

int main(void) { @autoreleasepool {
    TestWindow *window = [TestWindow new];
    window.frame = NSMakeRect(0, 0, 290, 298);
    NSScreen *primary = screen(NSMakeRect(0, 0, 1440, 900),
                              NSMakeRect(0, 80, 1440, 795), 2);
    NSScreen *left = screen(NSMakeRect(-1920, 0, 1920, 1080),
                           NSMakeRect(-1850, 0, 1850, 1055), 1);
    NSScreen *above = screen(NSMakeRect(0, 900, 2560, 1440),
                            NSMakeRect(0, 900, 2490, 1415), 1);
    NSArray<NSScreen *> *screens = @[primary, left, above];
    check(window, @[primary], NSMakePoint(300, 300), NSMakePoint(1134, 96));
    check(window, @[primary, left], NSMakePoint(-400, 300), NSMakePoint(-306, 16));
    check(window, screens, NSMakePoint(2200, 1400), NSMakePoint(2184, 916));
    // A cursor over the Dock still selects that screen by its full frame.
    check(window, screens, NSMakePoint(300, 20), NSMakePoint(1134, 96));
    check(window, screens, NSMakePoint(-1900, 300), NSMakePoint(-306, 16));
    check(window, screens, NSMakePoint(2530, 1400), NSMakePoint(2184, 916));
    // Shared edges, negative vertical origins, and a new selection each round.
    check(window, screens, NSMakePoint(0, 300), NSMakePoint(1134, 96));
    check(window, screens, NSMakePoint(300, 900), NSMakePoint(2184, 916));
    NSScreen *below = screen(NSMakeRect(-100, -1080, 1920, 1080),
                            NSMakeRect(-100, -1020, 1920, 1020), 1);
    check(window, @[primary, below], NSMakePoint(500, -500), NSMakePoint(1514, -1004));
    check(window, screens, NSMakePoint(-500, 300), NSMakePoint(-306, 16));
    check(window, screens, NSMakePoint(500, 300), NSMakePoint(1134, 96));
    // Refresh the list after disconnection; an unmatched point uses its first screen.
    check(window, @[primary], NSMakePoint(-500, 300), NSMakePoint(1134, 96));
    NSPoint previous = window.frame.origin;
    NSUInteger moves = window.moves;
    assert(!positionWindowAtPoint((NSWindow *)window, @[], NSZeroPoint, 16));
    assert(NSEqualPoints(window.frame.origin, previous) && window.moves == moves);
    NSScreen *tiny = screen(NSMakeRect(0, 0, 250, 240),
                           NSMakeRect(10, 20, 200, 200), 1);
    check(window, @[tiny], NSMakePoint(50, 50), NSMakePoint(10, 20));
    puts("Pantallas sintéticas 1/2/3, escalas 1x/2x, Dock, bordes, coordenadas negativas y desconexión: OK.");
    return 0;
}}
