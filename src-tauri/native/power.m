#import <AppKit/AppKit.h>
#include <time.h>
// CLOCK_UPTIME_RAW measures awake uptime, unlike a civil or continuous clock.
void sw_stop_cling(void);
uint64_t sw_awake_ns(void) { return clock_gettime_nsec_np(CLOCK_UPTIME_RAW); }
void sw_power_start(void (*notify)(int, uint64_t)) {
    NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
    // The center retains its block observers for the lifetime of this process.
    [center addObserverForName:NSWorkspaceWillSleepNotification object:nil queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *note){ (void)note; notify(1,sw_awake_ns()); sw_stop_cling(); }];
    [center addObserverForName:NSWorkspaceDidWakeNotification object:nil queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *note){ (void)note; notify(0,sw_awake_ns()); }];
    [center addObserverForName:NSWorkspaceWillPowerOffNotification object:nil queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *note){ (void)note; notify(2,sw_awake_ns()); sw_stop_cling(); }];
}

void sw_start_error(const char *detail) {
 @autoreleasepool {
    [NSApplication sharedApplication]; NSAlert *alert=[NSAlert new];
    alert.messageText=@"No se pudo abrir Stretch & Water";
    alert.informativeText=[NSString stringWithFormat:@"Descomprimí la carpeta completa en una ubicación propia con permiso de escritura y abrí la app desde allí.\n\n%@",[NSString stringWithUTF8String:detail]];
    [alert runModal];
 }
}
