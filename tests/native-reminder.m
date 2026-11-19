// Headless regression for the production timer and frame decoder. No window is created.
#import "../src-tauri/native/reminder.m"
#include <assert.h>

@interface TestPicture : NSObject
@property(nonatomic, strong) NSImage *image;
@end
@implementation TestPicture
- (NSRect)bounds { return NSMakeRect(0, 0, 270, 270); }
- (void)displayIfNeeded {}
@end

@interface TestRound : SWReminderRound
@property(nonatomic) BOOL onScreen;
@end
@implementation TestRound
- (BOOL)isOnScreen { return self.onScreen; }
@end

static void spin(double seconds) {
    NSDate *until = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while (until.timeIntervalSinceNow > 0) {
        [NSRunLoop.mainRunLoop runMode:NSDefaultRunLoopMode beforeDate:until];
    }
}
int main(int argc, char **argv) { @autoreleasepool {
    assert(argc == 2);
    NSURL *root = [NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]]];
    SWReminderRound *missing = [SWReminderRound new];
    missing.picture = (NSImageView *)[TestPicture new];
    assert(![missing displayFrame:0]);
    missing.reserveURL = [[root URLByDeletingLastPathComponent] URLByAppendingPathComponent:@"reserva.png"];
    assert([missing displayFrame:0] && missing.picture.image.isValid);
    for (int i = 0; i < 3; i++) { @autoreleasepool {
        NSString *kind = i == 1 ? @"water" : @"stretch";
        TestRound *r = [TestRound new];
        r.frames = [[SWReminderFrames alloc] initWithDirectory:[root URLByAppendingPathComponent:kind]];
        assert(r.frames.count == 241);
        r.picture = (NSImageView *)[TestPicture new];
        assert([r displayFrame:0] && r.picture.image != nil);
        r.token = i + 1;
        roundInWindow = r;
        assert(!sw_reminder_begin(i + 99,12000));
        assert(sw_reminder_begin(r.token,12000));
        spin(0.12);
        assert(!r.start && r.draws == 1);
        r.onScreen = YES;
        spin(1.48);
        assert(r.start && r.lastFrame == 0 && r.draws == 1);
        spin(4.0);
        assert(r.lastFrame > 85 && r.lastFrame < 115);
        spin(6.6);
        int drawn = 0, errors = -1;
        assert(sw_reminder_stop(&drawn,&errors) == 240);
        assert(drawn >= 210 && errors == 0 && !r.timer);
        printf("Ronda %d (%s): %d/241 poses dibujadas; primera pose antes del reloj, movimiento, última pose y limpieza OK.\n",i+1,kind.UTF8String,drawn);
        fflush(stdout);
    }}
    return 0;
}}
