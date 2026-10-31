// macOS: pictures written several output reports at a time. hidapi's hid_write calls
// IOHIDDeviceSetReport, which waits for each 320-byte report before the next; on the owner's deck a
// q98 key panel took 121-133 ms (2026-10-30, ~8 pictures a second). IOHIDDeviceSetReportWithCallback
// (IOKit's documented asynchronous form of the same call) keeps several in flight on a device
// object of our own, scheduled on a dispatch queue; the reports still go out in order on the
// device's output pipe. Used by pictures.rs. Built without ARC (build.rs): the dispatch objects
// live in a C struct and are released by hand.
#import <Foundation/Foundation.h>
#import <IOKit/IOKitLib.h>
#import <IOKit/hid/IOHIDDevice.h>
#include <stdatomic.h>

typedef struct {
    IOHIDDeviceRef device;
    dispatch_queue_t queue;
    // the writes in flight, and the first one that failed
    dispatch_group_t inflight;
    dispatch_semaphore_t free_slots;
    // signalled by the device's cancel handler
    dispatch_semaphore_t cancelled;
    atomic_int error;
    int depth;
} mh_pictures;

// One write's own copy of its report: freed when the write ends, even after a timeout.
typedef struct {
    mh_pictures *owner;
    uint8_t bytes[];
} mh_write;

static void write_done(void *context, IOReturn result, void *sender, IOHIDReportType type, uint32_t report_id, uint8_t *report, CFIndex length) {
    (void)sender; (void)type; (void)report_id; (void)report; (void)length;
    mh_write *w = context;
    mh_pictures *p = w->owner;
    if (result != kIOReturnSuccess) {
        int none = 0;
        atomic_compare_exchange_strong(&p->error, &none, result);
    }
    free(w);
    dispatch_semaphore_signal(p->free_slots);
    dispatch_group_leave(p->inflight);
}

static int write_reports(void *handle, const uint8_t *data, size_t len, size_t count, double timeout_s);

// Opens the HID service with this registry entry id (hidapi's `DevSrvsID:<id>` path) for writes,
// shared; NULL with *code set to IOKit's answer when it cannot be opened.
void *mh_pictures_open(uint64_t registry_id, int depth, int *code) {
    io_service_t service = IOServiceGetMatchingService(kIOMainPortDefault, IORegistryEntryIDMatching(registry_id));
    if (service == IO_OBJECT_NULL) {
        *code = kIOReturnNotFound;
        return NULL;
    }
    IOHIDDeviceRef device = IOHIDDeviceCreate(kCFAllocatorDefault, service);
    IOObjectRelease(service);
    if (device == NULL) {
        *code = kIOReturnNoDevice;
        return NULL;
    }
    IOReturn opened = IOHIDDeviceOpen(device, kIOHIDOptionsTypeNone);
    if (opened != kIOReturnSuccess) {
        CFRelease(device);
        *code = opened;
        return NULL;
    }
    mh_pictures *p = calloc(1, sizeof *p);
    p->device = device;
    p->queue = dispatch_queue_create("mh-device.pictures", DISPATCH_QUEUE_SERIAL);
    p->inflight = dispatch_group_create();
    p->depth = depth < 1 ? 1 : depth;
    p->free_slots = dispatch_semaphore_create(p->depth);
    IOHIDDeviceSetDispatchQueue(device, p->queue);
    // a device on a dispatch queue is cancelled before it is released (IOHIDDevice.h)
    p->cancelled = dispatch_semaphore_create(0);
    dispatch_semaphore_t cancelled = p->cancelled;
    IOHIDDeviceSetCancelHandler(device, ^{
        dispatch_semaphore_signal(cancelled);
    });
    IOHIDDeviceActivate(device);
    *code = kIOReturnSuccess;
    return p;
}

// Writes `count` reports of `len` bytes each (back to back in `data`, the report id in each first
// byte), up to the depth at a time, and returns when every one has ended: 0, or the first failure
// as IOKit's code; kIOReturnTimeout when they did not end in `timeout_s`.
int mh_pictures_write(void *handle, const uint8_t *data, size_t len, size_t count, double timeout_s) {
    // the caller is a Rust thread with no pool of its own: what IOKit autoreleases for each call
    // would pile up (11 MB a minute at 19 pictures a second, 2026-10-30)
    @autoreleasepool {
        return write_reports(handle, data, len, count, timeout_s);
    }
}

static int write_reports(void *handle, const uint8_t *data, size_t len, size_t count, double timeout_s) {
    mh_pictures *p = handle;
    atomic_store(&p->error, 0);
    dispatch_time_t deadline = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeout_s * NSEC_PER_SEC));
    int result = 0;
    for (size_t i = 0; i < count; i++) {
        if (dispatch_semaphore_wait(p->free_slots, deadline) != 0) {
            result = kIOReturnTimeout;
            break;
        }
        int failed = atomic_load(&p->error);
        if (failed != 0) {
            dispatch_semaphore_signal(p->free_slots);
            result = failed;
            break;
        }
        mh_write *w = malloc(sizeof *w + len);
        w->owner = p;
        memcpy(w->bytes, data + i * len, len);
        dispatch_group_enter(p->inflight);
        IOReturn sent = IOHIDDeviceSetReportWithCallback(p->device, kIOHIDReportTypeOutput, w->bytes[0], w->bytes, (CFIndex)len, timeout_s, write_done, w);
        if (sent != kIOReturnSuccess) {
            free(w);
            dispatch_semaphore_signal(p->free_slots);
            dispatch_group_leave(p->inflight);
            result = sent;
            break;
        }
    }
    // every write ends before the next picture
    if (dispatch_group_wait(p->inflight, deadline) != 0 && result == 0) {
        result = kIOReturnTimeout;
    }
    if (result == 0) {
        result = atomic_load(&p->error);
    }
    return result;
}

void mh_pictures_close(void *handle) {
    mh_pictures *p = handle;
    dispatch_group_wait(p->inflight, dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC));
    IOHIDDeviceCancel(p->device);
    dispatch_semaphore_wait(p->cancelled, dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC));
    IOHIDDeviceClose(p->device, kIOHIDOptionsTypeNone);
    CFRelease(p->device);
    dispatch_release(p->cancelled);
    dispatch_release(p->free_slots);
    dispatch_release(p->inflight);
    dispatch_release(p->queue);
    free(p);
}
