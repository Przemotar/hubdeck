// Re-enumerate the USB device behind a HID service, as pulling the module off the base and back
// does: a keys or dial module whose firmware stopped taking commands comes back clean. IOUSBHost
// (macOS 10.15+), no root and no entitlement: the device object is opened without capture.
#import <Foundation/Foundation.h>
#import <IOKit/IOKitLib.h>
#import <IOUSBHost/IOUSBHost.h>

// Returns 0 on success, otherwise a code: 1 no such HID service, 2 no USB device above it,
// 3 the device could not be opened (busy), 4 the reset failed.
int mh_usb_reenumerate(uint64_t hid_registry_id) {
    @autoreleasepool {
        io_service_t entry = IOServiceGetMatchingService(kIOMainPortDefault, IORegistryEntryIDMatching(hid_registry_id));
        if (entry == IO_OBJECT_NULL) return 1;
        // walk up the service plane to the USB device that holds this HID interface
        for (int i = 0; i < 8 && entry != IO_OBJECT_NULL && !IOObjectConformsTo(entry, "IOUSBHostDevice"); i++) {
            io_registry_entry_t parent = IO_OBJECT_NULL;
            kern_return_t kr = IORegistryEntryGetParentEntry(entry, kIOServicePlane, &parent);
            IOObjectRelease(entry);
            entry = kr == KERN_SUCCESS ? parent : IO_OBJECT_NULL;
        }
        if (entry == IO_OBJECT_NULL || !IOObjectConformsTo(entry, "IOUSBHostDevice")) {
            if (entry != IO_OBJECT_NULL) IOObjectRelease(entry);
            return 2;
        }
        NSError *error = nil;
        IOUSBHostDevice *device = [[IOUSBHostDevice alloc] initWithIOService:entry options:IOUSBHostObjectInitOptionsNone queue:nil error:&error interestHandler:nil];
        IOObjectRelease(entry);
        if (device == nil) {
            NSLog(@"mh-device: open USB device: %@", error);
            return 3;
        }
        BOOL ok = [device resetWithError:&error];
        if (!ok) NSLog(@"mh-device: reset USB device: %@", error);
        [device destroy];
        return ok ? 0 : 4;
    }
}
