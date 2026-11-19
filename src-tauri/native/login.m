// Linked only into NORMAL. No login service is invoked by builds or tests.
#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <unistd.h>
#include <stdio.h>

static NSString *const label = @"com.stretchandwater.poc001.login";
static NSString *const bundleID = @"com.stretchandwater.poc001";
static NSError *failure(NSString *s) {
    return [NSError errorWithDomain:@"StretchWaterLogin" code:1
                           userInfo:@{NSLocalizedDescriptionKey:s}];
}
// Legacy (macOS 12): all launchctl operations address our own label or plist.
// The job opens the signed bundle; unloading it does not terminate the app.
static BOOL ctl(NSArray<NSString *> *args) {
    NSTask *task=[NSTask new]; task.executableURL=[NSURL fileURLWithPath:@"/bin/launchctl"];
    task.arguments=args;task.standardOutput=NSFileHandle.fileHandleWithNullDevice;
    task.standardError=NSFileHandle.fileHandleWithNullDevice;
    if (![task launchAndReturnError:nil]) return NO;
    [task waitUntilExit]; return task.terminationStatus==0;
}
static int legacy(int operation,NSBundle *bundle,NSError **error) {
    NSFileManager *fm=NSFileManager.defaultManager;
    NSURL *agents=[[NSURL fileURLWithPath:NSHomeDirectory() isDirectory:YES] URLByAppendingPathComponent:@"Library/LaunchAgents" isDirectory:YES];
    NSURL *url=[agents URLByAppendingPathComponent:[label stringByAppendingString:@".plist"]];
    NSString *domain=[NSString stringWithFormat:@"gui/%u",getuid()];
    NSString *target=[domain stringByAppendingFormat:@"/%@",label];
    NSDictionary *config=@{@"Label":label,@"ProgramArguments":@[@"/usr/bin/open",@"-g",bundle.bundlePath],
        @"RunAtLoad":@YES,@"LimitLoadToSessionType":@"Aqua"};
    BOOL exists=[fm fileExistsAtPath:url.path];
    if (exists && ![[NSDictionary dictionaryWithContentsOfURL:url] isEqual:config]) {
        *error=failure(@"Entrada propia modificada o asociada a otra ubicación.");return -1;
    }
    BOOL loaded=ctl(@[@"print",target]);
    if (operation==1) {
        if(!exists && (![fm createDirectoryAtURL:agents withIntermediateDirectories:YES attributes:nil error:error]
                    || ![config writeToURL:url atomically:YES])) {
            if(!*error)*error=failure(@"No se pudo guardar la entrada de inicio.");return -1;
        }
        if (!ctl(@[@"enable",target]) || (!loaded && !ctl(@[@"bootstrap",domain,url.path]))) {
            *error=failure(@"macOS no confirmó el registro de inicio.");return -1;
        }
    } else if(operation==2) {
        if(loaded && !ctl(@[@"bootout",target])){*error=failure(@"No se pudo descargar la entrada propia.");return -1;}
        if(exists && ![fm removeItemAtURL:url error:error])return -1;
    }
    exists=[fm fileExistsAtPath:url.path];loaded=ctl(@[@"print",target]);
    if(!exists && !loaded)return 0;
    if(exists && loaded)return 1;
    *error=failure(@"El archivo y el registro de la sesión no coinciden.");return -1;
}
// 0 = query, 1 = register, 2 = unregister; result is the observed state.
int sw_login(int operation,char *message,size_t capacity) {
 @autoreleasepool {
    NSError *error=nil;int state=-1;
    NSBundle *bundle=NSBundle.mainBundle;
    if (![bundle.bundleIdentifier isEqual:bundleID] ||
        ![bundle.bundleURL.lastPathComponent isEqual:@"Stretch & Water.app"]) {
        error=failure(@"Solo se admite el paquete normal del proyecto.");
    } else if (@available(macOS 13.0,*)) {
        SMAppService *service=SMAppService.mainAppService;
        if(operation==1 && service.status!=SMAppServiceStatusEnabled) [service registerAndReturnError:&error];
        if(operation==2 && service.status!=SMAppServiceStatusNotRegistered) [service unregisterAndReturnError:&error];
        switch(service.status){
            case SMAppServiceStatusEnabled:state=1;break;
            case SMAppServiceStatusNotRegistered:state=0;break;
            case SMAppServiceStatusRequiresApproval:state=2;break;
            case SMAppServiceStatusNotFound: error=error ?: failure(@"macOS no encuentra el servicio de esta app.");break;
            default:break;
        }
    } else {state=legacy(operation,bundle,&error);}
    if(error && capacity)snprintf(message,capacity,"%s",error.localizedDescription.UTF8String);
    return state;
 }
}
