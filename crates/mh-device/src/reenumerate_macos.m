// USB requests to the device behind a HID service, through IOUSBHost (macOS 10.15+), no root and no
// entitlement: the device object is opened without capture.
// - Re-enumerate, as pulling the module off the base and back does: a keys or dial module whose
//   firmware stopped taking commands comes back clean.
// - SET_IDLE(0), which Windows' HID driver sends to every HID device and macOS does not: the
//   rollers module (firmware 01.00.03.01) sends no input at all until it gets one (2026-10-15).
#import <Foundation/Foundation.h>
#import <IOKit/IOKitLib.h>
#import <IOUSBHost/IOUSBHost.h>

// The USB device above a HID service, opened; nil with *code set: 1 no such HID service, 2 no
// USB device above it, 3 the device could not be opened (busy).
static IOUSBHostDevice *open_device(uint64_t hid_registry_id, int *code) {
    io_service_t entry = IOServiceGetMatchingService(kIOMainPortDefault, IORegistryEntryIDMatching(hid_registry_id));
    if (entry == IO_OBJECT_NULL) {
        *code = 1;
        return nil;
    }
    // walk up the service plane to the USB device that holds this HID interface
    for (int i = 0; i < 8 && entry != IO_OBJECT_NULL && !IOObjectConformsTo(entry, "IOUSBHostDevice"); i++) {
        io_registry_entry_t parent = IO_OBJECT_NULL;
        kern_return_t kr = IORegistryEntryGetParentEntry(entry, kIOServicePlane, &parent);
        IOObjectRelease(entry);
        entry = kr == KERN_SUCCESS ? parent : IO_OBJECT_NULL;
    }
    if (entry == IO_OBJECT_NULL || !IOObjectConformsTo(entry, "IOUSBHostDevice")) {
        if (entry != IO_OBJECT_NULL) IOObjectRelease(entry);
        *code = 2;
        return nil;
    }
    NSError *error = nil;
    IOUSBHostDevice *device = [[IOUSBHostDevice alloc] initWithIOService:entry options:IOUSBHostObjectInitOptionsNone queue:nil error:&error interestHandler:nil];
    IOObjectRelease(entry);
    if (device == nil) {
        NSLog(@"mh-device: open USB device: %@", error);
        *code = 3;
    }
    return device;
}

// Returns 0 on success, otherwise the open_device code or 4: the reset failed.
int mh_usb_reenumerate(uint64_t hid_registry_id) {
    @autoreleasepool {
        int code = 0;
        IOUSBHostDevice *device = open_device(hid_registry_id, &code);
        if (device == nil) return code;
        NSError *error = nil;
        BOOL ok = [device resetWithError:&error];
        if (!ok) NSLog(@"mh-device: reset USB device: %@", error);
        [device destroy];
        return ok ? 0 : 4;
    }
}

// HID class request SET_IDLE (duration 0 = report only on change, all reports) to interface 0.
// Returns 0 on success, otherwise the open_device code or 4: the request failed.
int mh_usb_set_idle(uint64_t hid_registry_id) {
    @autoreleasepool {
        int code = 0;
        IOUSBHostDevice *device = open_device(hid_registry_id, &code);
        if (device == nil) return code;
        IOUSBDeviceRequest request = {.bmRequestType = 0x21, .bRequest = 0x0A, .wValue = 0, .wIndex = 0, .wLength = 0};
        NSError *error = nil;
        BOOL ok = [device sendDeviceRequest:request error:&error];
        if (!ok) NSLog(@"mh-device: SET_IDLE: %@", error);
        [device destroy];
        return ok ? 0 : 4;
    }
}
