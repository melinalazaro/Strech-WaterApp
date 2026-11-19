#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import "reminder_frames.h"

// One presentation, confined to the macOS main thread. A fresh instance owns
// the poster, timer and monotonic clock for each automatic or manual reminder.
@interface SWReminderRound : NSObject
@property(nonatomic, strong) SWReminderFrames *frames;
@property(nonatomic, strong) NSURL *reserveURL;
@property(nonatomic, strong) NSView *overlay;
@property(nonatomic, strong) NSImageView *picture;
@property(nonatomic, strong) NSTimer *timer;
@property(nonatomic) uint64_t token;
@property(nonatomic) NSUInteger lastFrame;
@property(nonatomic) int draws;
@property(nonatomic) int readErrors;
@property(nonatomic) CFTimeInterval start;
@property(nonatomic) double intervalMs;
- (BOOL)displayFrame:(NSUInteger)index;
- (BOOL)isOnScreen;
- (void)startForMs:(double)durationMs;
- (void)tick:(NSTimer *)timer;
@end

@implementation SWReminderRound
- (BOOL)displayFrame:(NSUInteger)index {
    CGImageRef pixels = [self.frames copyFrame:index];
    if (!pixels) {
        self.readErrors++;
        self.frames = nil;
        if (!self.reserveURL) return NO;
        NSImage *reserve = [[NSImage alloc] initWithContentsOfURL:self.reserveURL];
        if (!reserve.isValid) return NO;
        self.picture.image = reserve;
        [self.picture displayIfNeeded];
        return YES;
    }
    self.picture.image = [[NSImage alloc] initWithCGImage:pixels size:self.picture.bounds.size];
    CGImageRelease(pixels);
    [self.picture displayIfNeeded];
    self.lastFrame = index;
    self.draws++;
    return YES;
}
- (BOOL)isOnScreen { return self.overlay.window.isVisible; }
- (void)startForMs:(double)durationMs {
    self.intervalMs = durationMs;
    NSTimer *timer = [NSTimer timerWithTimeInterval:1.0 / 60.0 target:self selector:@selector(tick:) userInfo:nil repeats:YES];
    self.timer = timer;
    [NSRunLoop.mainRunLoop addTimer:timer forMode:NSRunLoopCommonModes];
}
- (void)tick:(NSTimer *)timer {
    if (timer != self.timer) return;
    if (!self.start) {
        if (![self isOnScreen]) return;
        [self.overlay displayIfNeeded];
        [CATransaction flush];
        self.start = CACurrentMediaTime();
    }
    double elapsedMs = (CACurrentMediaTime() - self.start) * 1000;
    NSUInteger next = [self.frames frameAtElapsedMs:elapsedMs];
    if (self.frames && next != self.lastFrame) [self displayFrame:next];
    if (elapsedMs >= self.intervalMs - 250) {
        self.overlay.alphaValue = MAX(0.0, MIN(1.0, (self.intervalMs - elapsedMs) / 250.0));
    }
    if (elapsedMs >= self.intervalMs) {
        [timer invalidate];
        self.timer = nil;
    }
}
@end

static SWReminderRound *roundInWindow;

static NSRect pictureFrame(BOOL stretch) {
    CGFloat side = stretch ? 270 : 180;
    // At these sizes the visible lower edge is 27 pt inside stretch, 4 pt inside water.
    CGFloat y = 26 - (stretch ? 23 : 0);
    return NSMakeRect(10, y, side, side);
}

// The host can be flipped. The reminder's own view is unflipped, so the
// picture and label keep the same local coordinates at both sizes.
static NSView *anchoredReminderView(NSView *host, CGFloat side) {
    NSRect bounds = host.bounds;
    CGFloat width = side + 20, height = side + 28;
    CGFloat y = host.isFlipped ? NSMaxY(bounds) - height : NSMinY(bounds);
    NSView *view = [[NSView alloc] initWithFrame:NSMakeRect(NSMaxX(bounds) - width, y, width, height)];
    view.autoresizingMask = NSViewMinXMargin | (host.isFlipped ? NSViewMinYMargin : NSViewMaxYMargin);
    return view;
}

// Cursor, screen bounds and window frames all use AppKit points. Keeping this
// calculation in that coordinate system also handles mixed Retina scales.
static BOOL positionWindowAtPoint(NSWindow *window, NSArray<NSScreen *> *screens,
                                  NSPoint cursor, CGFloat margin) {
    NSScreen *selected = screens.firstObject;
    for (NSScreen *screen in screens) {
        if (NSPointInRect(cursor, screen.frame)) {
            selected = screen;
            break;
        }
    }
    if (!selected) return NO;
    NSRect area = selected.visibleFrame;
    NSSize size = window.frame.size;
    CGFloat x = MAX(NSMinX(area), NSMaxX(area) - size.width - margin);
    CGFloat y = NSMinY(area) + MAX(0, MIN(margin, NSHeight(area) - size.height));
    [window setFrameOrigin:NSMakePoint(x, y)];
    return YES;
}

int sw_reminder_position_at_cursor(void *rawWindow, double margin) {
    if (!rawWindow || !NSThread.isMainThread) return 0;
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    // Take a fresh snapshot immediately before each Show, including queued
    // reminders and screen changes between rounds. Playback stays in place.
    return positionWindowAtPoint(window, NSScreen.screens, NSEvent.mouseLocation, margin);
}

int sw_reminder_stop(int *draws, int *errors) {
    SWReminderRound *old = roundInWindow;
    roundInWindow = nil;
    [old.timer invalidate];
    old.timer = nil;
    [old.overlay removeFromSuperview];
    if (draws) *draws = old.draws;
    if (errors) *errors = old.readErrors;
    return old ? (int)old.lastFrame : -1;
}

int sw_reminder_prepare(void *rawWindow, uint64_t token, int stretch, int testProfile) {
    sw_reminder_stop(NULL, NULL);
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    if (!window.contentView || !NSThread.isMainThread) return 0;
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    NSString *folder = stretch ? @"films/stretch" : @"films/water";
    NSURL *directory = [NSBundle.mainBundle.resourceURL URLByAppendingPathComponent:folder isDirectory:YES];
    SWReminderRound *round = [SWReminderRound new];
    round.token = token;
    round.frames = [[SWReminderFrames alloc] initWithDirectory:directory];
    round.reserveURL = [NSBundle.mainBundle URLForResource:@"reserva" withExtension:@"png"];
    NSRect frame = pictureFrame(stretch);
    CGFloat side = NSWidth(frame);
    NSView *overlay = anchoredReminderView(window.contentView, side);
    round.overlay = overlay;
    NSImageView *picture = [[NSImageView alloc] initWithFrame:frame];
    picture.imageScaling = NSImageScaleProportionallyUpOrDown;
    round.picture = picture;
    [overlay addSubview:picture];
    NSString *caption = stretch ? @"Estiramiento" : @"Agua";
    if (testProfile) caption = [@"PRUEBA · " stringByAppendingString:caption];
    NSTextField *label = [NSTextField labelWithString:caption];
    label.frame = NSMakeRect(0, 2, side + 20, 20);
    label.alignment = NSTextAlignmentCenter;
    label.font = [NSFont systemFontOfSize:12 weight:NSFontWeightSemibold];
    label.textColor = NSColor.whiteColor;
    NSShadow *shadow = [NSShadow new];
    shadow.shadowColor = NSColor.blackColor;
    shadow.shadowBlurRadius = 2;
    shadow.shadowOffset = NSMakeSize(0, -1);
    label.shadow = shadow;
    [overlay addSubview:label];
    if (![round displayFrame:0]) return 0;
    [window.contentView addSubview:overlay positioned:NSWindowAbove relativeTo:nil];
    roundInWindow = round;
    return round.frames ? 1 : 2;
}

int sw_reminder_begin(uint64_t token, uint64_t durationMs) {
    SWReminderRound *round = roundInWindow;
    if (!round || round.token != token || !durationMs) return 0;
    if (round.timer) return 1;
    [round startForMs:durationMs];
    return 1;
}
