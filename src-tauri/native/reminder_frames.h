#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

@interface SWReminderFrames : NSObject
@property(nonatomic, readonly) NSUInteger count;
- (instancetype)initWithDirectory:(NSURL *)directory;
- (NSUInteger)frameAtElapsedMs:(double)elapsedMs;
- (CGImageRef)copyFrame:(NSUInteger)index CF_RETURNS_RETAINED;
@end
