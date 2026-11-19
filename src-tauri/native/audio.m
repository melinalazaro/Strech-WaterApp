#import <AppKit/AppKit.h>

// Called on the main thread. Play starts asynchronously on the default output.
static NSSound *cling;
int sw_play_cling(void) {
    if (!cling) {
        NSURL *url = [[NSBundle mainBundle] URLForResource:@"cling" withExtension:@"wav"];
        if (!url) return 0;
        cling = [[NSSound alloc] initWithContentsOfURL:url byReference:NO];
        if (!cling) return 0;
        cling.loops = NO;
    }
    cling.currentTime = 0;
    return [cling play] ? 1 : 0;
}

void sw_stop_cling(void) {
    [cling stop];
}
