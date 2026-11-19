// Exercise AppKit's view geometry without opening a window.
#import "../src-tauri/native/reminder.m"
#include <assert.h>

@interface FlippedHost : NSView
@end
@implementation FlippedHost
- (BOOL)isFlipped { return YES; }
@end

static void checkRounds(NSView *host) {
    CGRect firstWater = CGRectNull;
    for (NSNumber *n in @[@180, @270, @180, @180, @270, @180]) {
        CGFloat side = n.doubleValue;
        NSView *overlay = anchoredReminderView(host, side);
        [host addSubview:overlay];
        BOOL stretch = (side == 270);
        NSRect picture = [overlay convertRect:pictureFrame(stretch) toView:host];
        assert(NSWidth(picture) == side && NSHeight(picture) == side);
        assert(NSMaxX(host.bounds) - NSMaxX(picture) == 10);
        CGFloat bottom = host.isFlipped ? NSMaxY(host.bounds) - NSMaxY(picture)
                                        : NSMinY(picture) - NSMinY(host.bounds);
        assert(bottom + (stretch ? 27 : 4) == 30);
        if (side == 180) {
            if (CGRectIsNull(firstWater)) firstWater = picture;
            assert(CGRectEqualToRect(firstWater, picture));
        }
        [overlay removeFromSuperview];
    }
}

int main(void) { @autoreleasepool {
    NSRect frame = NSMakeRect(0, 0, 290, 298);
    checkRounds([[NSView alloc] initWithFrame:frame]);
    checkRounds([[FlippedHost alloc] initWithFrame:frame]);
    puts("Primera agua, repetición y alternancia: mismo rectángulo en ambos sistemas de coordenadas AppKit.");
    return 0;
}}
