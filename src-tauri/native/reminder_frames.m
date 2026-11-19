#import "reminder_frames.h"
#import <ImageIO/ImageIO.h>

@implementation SWReminderFrames {
    NSURL *_directory;
    NSArray<NSNumber *> *_ends;
}
- (instancetype)initWithDirectory:(NSURL *)directory {
    self = [super init];
    if (!self) return nil;
    NSData *data = [NSData dataWithContentsOfURL:[directory URLByAppendingPathComponent:@"durations.json"]];
    NSArray *durations = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    if (![durations isKindOfClass:NSArray.class] || !durations.count) return nil;
    NSMutableArray *ends = [NSMutableArray arrayWithCapacity:durations.count];
    double total = 0;
    for (NSNumber *duration in durations) {
        if (![duration isKindOfClass:NSNumber.class] || duration.doubleValue <= 0) return nil;
        total += duration.doubleValue;
        [ends addObject:@(total)];
    }
    _directory = directory;
    _ends = [ends copy];
    return self;
}
- (NSUInteger)count { return _ends.count; }
- (NSUInteger)frameAtElapsedMs:(double)elapsedMs {
    // The poster has already been drawn before the window opens.
    double motionMs = elapsedMs - 1500.0;
    NSUInteger lo = 0, hi = _ends.count - 1;
    while (lo < hi) {
        NSUInteger mid = lo + (hi - lo) / 2;
        if (motionMs < _ends[mid].doubleValue) hi = mid;
        else lo = mid + 1;
    }
    return lo;
}
- (CGImageRef)copyFrame:(NSUInteger)index {
    if (index >= _ends.count) return NULL;
    NSString *name = [NSString stringWithFormat:@"%03lu.png", (unsigned long)index];
    NSURL *url = [_directory URLByAppendingPathComponent:name];
    CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)url, NULL);
    if (!source) return NULL;
    NSDictionary *options = @{(__bridge NSString *)kCGImageSourceShouldCacheImmediately:@YES};
    CGImageRef image = CGImageSourceCreateImageAtIndex(source, 0, (__bridge CFDictionaryRef)options);
    CFRelease(source);
    return image;
}
@end
