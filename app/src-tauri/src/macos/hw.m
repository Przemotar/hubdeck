// macOS hardware readings and monitor brightness for HubDeck (owner, 2026-10-13: "rób wszystko
// teraz"). Apple gives no public API for these, so this uses the same system interfaces the well
// known open-source tools use (Stats / macmon for sensors, MonitorControl for brightness), each one
// looked up at run time: a Mac without one gets "not available" instead of a crash or a link error.
//
//   temperatures: IOHIDEventSystemClient (temperature sensors, Apple silicon)
//   GPU load:     IOAccelerator's PerformanceStatistics (public IOKit)
//   built-in screen brightness: DisplayServices (private framework)
//   external monitors (Apple silicon): DDC/CI over IOAVService (VCP 0x10, brightness)
#import <CoreFoundation/CoreFoundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <IOKit/IOKitLib.h>
#include <dlfcn.h>
#include <string.h>
#include <unistd.h>

// ---------- temperatures ----------

typedef void *HIDClient;
typedef void *HIDService;
typedef void *HIDEvent;

static void *iokit(void) {
    static void *h;
    if (!h) h = dlopen("/System/Library/Frameworks/IOKit.framework/IOKit", RTLD_LAZY);
    return h;
}

/// Average temperature of the sensors whose names contain any of `parts` (NULL-terminated);
/// -1 when there is none.
static double temperature(const char **parts) {
    HIDClient (*create)(CFAllocatorRef) = dlsym(iokit(), "IOHIDEventSystemClientCreate");
    int (*set_matching)(HIDClient, CFDictionaryRef) = dlsym(iokit(), "IOHIDEventSystemClientSetMatching");
    CFArrayRef (*services)(HIDClient) = dlsym(iokit(), "IOHIDEventSystemClientCopyServices");
    CFTypeRef (*property)(HIDService, CFStringRef) = dlsym(iokit(), "IOHIDServiceClientCopyProperty");
    HIDEvent (*copy_event)(HIDService, int64_t, int32_t, int64_t) = dlsym(iokit(), "IOHIDServiceClientCopyEvent");
    double (*float_value)(HIDEvent, int32_t) = dlsym(iokit(), "IOHIDEventGetFloatValue");
    if (!create || !set_matching || !services || !property || !copy_event || !float_value) return -1;

    HIDClient client = create(kCFAllocatorDefault);
    if (!client) return -1;
    int page = 0xff00, usage = 5;
    CFNumberRef p = CFNumberCreate(NULL, kCFNumberIntType, &page), u = CFNumberCreate(NULL, kCFNumberIntType, &usage);
    const void *keys[] = {CFSTR("PrimaryUsagePage"), CFSTR("PrimaryUsage")};
    const void *values[] = {p, u};
    CFDictionaryRef matching = CFDictionaryCreate(NULL, keys, values, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    set_matching(client, matching);
    CFRelease(matching);
    CFRelease(p);
    CFRelease(u);

    double sum = 0;
    int n = 0;
    CFArrayRef list = services(client);
    if (list) {
        for (CFIndex i = 0; i < CFArrayGetCount(list); i++) {
            HIDService s = (HIDService)CFArrayGetValueAtIndex(list, i);
            CFStringRef name = property(s, CFSTR("Product"));
            if (!name) continue;
            char buf[128] = {0};
            CFStringGetCString(name, buf, sizeof buf, kCFStringEncodingUTF8);
            CFRelease(name);
            int match = 0;
            for (const char **part = parts; *part; part++) {
                // "TP*g": the M2-and-later GPU sensors ("PMU TP1g", "PMU TP2g"...)
                if (strcmp(*part, "TP*g") == 0) {
                    size_t l = strlen(buf);
                    match |= strncmp(buf, "PMU TP", 6) == 0 && l > 6 && buf[l - 1] == 'g';
                } else {
                    match |= strstr(buf, *part) != NULL;
                }
            }
            if (!match) continue;
            HIDEvent e = copy_event(s, 15 /* temperature */, 0, 0);
            if (!e) continue;
            double t = float_value(e, 15 << 16);
            CFRelease(e);
            if (t > 0 && t < 150) {
                sum += t;
                n++;
            }
        }
        CFRelease(list);
    }
    CFRelease(client);
    return n ? sum / n : -1;
}

/// The temperature sensors' names, one per line, into `out` (diagnostics: names differ by chip).
void mh_sensor_names(char *out, int size) {
    out[0] = 0;
    HIDClient (*create)(CFAllocatorRef) = dlsym(iokit(), "IOHIDEventSystemClientCreate");
    int (*set_matching)(HIDClient, CFDictionaryRef) = dlsym(iokit(), "IOHIDEventSystemClientSetMatching");
    CFArrayRef (*services)(HIDClient) = dlsym(iokit(), "IOHIDEventSystemClientCopyServices");
    CFTypeRef (*property)(HIDService, CFStringRef) = dlsym(iokit(), "IOHIDServiceClientCopyProperty");
    if (!create || !set_matching || !services || !property) return;
    HIDClient client = create(kCFAllocatorDefault);
    int page = 0xff00, usage = 5;
    CFNumberRef p = CFNumberCreate(NULL, kCFNumberIntType, &page), u = CFNumberCreate(NULL, kCFNumberIntType, &usage);
    const void *keys[] = {CFSTR("PrimaryUsagePage"), CFSTR("PrimaryUsage")};
    const void *values[] = {p, u};
    CFDictionaryRef matching = CFDictionaryCreate(NULL, keys, values, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    set_matching(client, matching);
    CFRelease(matching);
    CFRelease(p);
    CFRelease(u);
    CFArrayRef list = services(client);
    for (CFIndex i = 0; list && i < CFArrayGetCount(list); i++) {
        CFStringRef name = property((HIDService)CFArrayGetValueAtIndex(list, i), CFSTR("Product"));
        if (!name) continue;
        char buf[128] = {0};
        CFStringGetCString(name, buf, sizeof buf, kCFStringEncodingUTF8);
        CFRelease(name);
        if ((int)(strlen(out) + strlen(buf) + 2) < size) {
            strcat(out, buf);
            strcat(out, "\n");
        }
    }
    if (list) CFRelease(list);
    CFRelease(client);
}

double mh_cpu_temp(void) {
    // Apple silicon: the CPU clusters' sensors ("pACC MTR", "eACC MTR"), else the dies ("tdie")
    const char *clusters[] = {"ACC MTR", NULL};
    double t = temperature(clusters);
    if (t >= 0) return t;
    const char *dies[] = {"tdie", NULL};
    return temperature(dies);
}

double mh_gpu_temp(void) {
    // M1: "GPU MTR Temp Sensor"; M2 and later: "PMU TP<n>g"
    const char *gpu[] = {"GPU MTR", "TP*g", NULL};
    return temperature(gpu);
}

// ---------- GPU load ----------

double mh_gpu_load(void) {
    io_iterator_t it = 0;
    if (IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching("IOAccelerator"), &it) != KERN_SUCCESS) return -1;
    double load = -1;
    io_object_t s;
    while ((s = IOIteratorNext(it)) && load < 0) {
        CFDictionaryRef stats = IORegistryEntryCreateCFProperty(s, CFSTR("PerformanceStatistics"), kCFAllocatorDefault, 0);
        if (stats && CFGetTypeID(stats) == CFDictionaryGetTypeID()) {
            CFNumberRef v = CFDictionaryGetValue(stats, CFSTR("Device Utilization %"));
            if (!v) v = CFDictionaryGetValue(stats, CFSTR("GPU Activity(%)"));
            int64_t x = 0;
            if (v && CFNumberGetValue(v, kCFNumberSInt64Type, &x)) load = (double)x;
        }
        if (stats) CFRelease(stats);
        IOObjectRelease(s);
    }
    IOObjectRelease(it);
    return load;
}

// ---------- brightness ----------

static void *display_services(void) {
    static void *h;
    if (!h) h = dlopen("/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices", RTLD_LAZY);
    return h;
}

static CGDirectDisplayID builtin_display(void) {
    CGDirectDisplayID ids[16];
    uint32_t n = 0;
    if (CGGetOnlineDisplayList(16, ids, &n) != kCGErrorSuccess) return 0;
    for (uint32_t i = 0; i < n; i++)
        if (CGDisplayIsBuiltin(ids[i])) return ids[i];
    return 0;
}

/// External monitors' DDC services (Apple silicon), in registry order; up to `max`.
typedef CFTypeRef AVService;
static int external_services(AVService *out, int max) {
    AVService (*create)(CFAllocatorRef, io_service_t) = dlsym(iokit(), "IOAVServiceCreateWithService");
    if (!create) return 0;
    io_iterator_t it = 0;
    if (IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching("DCPAVServiceProxy"), &it) != KERN_SUCCESS) return 0;
    int n = 0;
    io_object_t s;
    while ((s = IOIteratorNext(it))) {
        CFStringRef where = IORegistryEntryCreateCFProperty(s, CFSTR("Location"), kCFAllocatorDefault, 0);
        int external = where && CFGetTypeID(where) == CFStringGetTypeID() && CFStringCompare(where, CFSTR("External"), 0) == kCFCompareEqualTo;
        if (where) CFRelease(where);
        if (external && n < max) {
            AVService av = create(kCFAllocatorDefault, s);
            if (av) out[n++] = av;
        }
        IOObjectRelease(s);
    }
    IOObjectRelease(it);
    return n;
}

static int ddc_write(AVService av, const uint8_t *send, int len) {
    int (*write_i2c)(AVService, uint32_t, uint32_t, void *, uint32_t) = dlsym(iokit(), "IOAVServiceWriteI2C");
    if (!write_i2c) return -1;
    uint8_t packet[16];
    packet[0] = 0x80 | (uint8_t)(len + 1);
    packet[1] = (uint8_t)len;
    memcpy(packet + 2, send, len);
    // the length byte doubles as the opcode (1 = get VCP, 3 = set VCP); the checksum starts from
    // the host address, plus the sub-address for a write (as MonitorControl does)
    uint8_t chk = len == 1 ? 0x6E : 0x6E ^ 0x51;
    for (int i = 0; i < len + 2; i++) chk ^= packet[i];
    packet[len + 2] = chk;
    int r = 0;
    // monitors miss single writes now and then: twice, as MonitorControl does
    for (int i = 0; i < 2; i++) {
        usleep(10000);
        r = write_i2c(av, 0x37, 0x51, packet, (uint32_t)(len + 3));
    }
    return r;
}

/// 0..100, or -1.
static int ddc_get(AVService av) {
    int (*read_i2c)(AVService, uint32_t, uint32_t, void *, uint32_t) = dlsym(iokit(), "IOAVServiceReadI2C");
    if (!read_i2c) return -1;
    const uint8_t ask[] = {0x10};
    if (ddc_write(av, ask, 1) != 0) return -1;
    usleep(50000);
    uint8_t reply[12] = {0};
    if (read_i2c(av, 0x37, 0x51, reply, sizeof reply) != 0) return -1;
    for (int i = 0; i + 7 < (int)sizeof reply; i++) {
        if (reply[i] == 0x02 && reply[i + 2] == 0x10) {
            int max = reply[i + 4] << 8 | reply[i + 5], cur = reply[i + 6] << 8 | reply[i + 7];
            if (max > 0) return cur * 100 / max;
        }
    }
    return -1;
}

/// How many monitors HubDeck can set: the built-in screen first, then external ones.
int mh_monitors(void) {
    AVService avs[8];
    int ext = external_services(avs, 8);
    for (int i = 0; i < ext; i++) CFRelease(avs[i]);
    return (builtin_display() && dlsym(display_services(), "DisplayServicesGetBrightness") ? 1 : 0) + ext;
}

/// Monitor `index` (0 = the first): 0..100, or -1 when it cannot be read.
int mh_monitor_get(int index) {
    int has_builtin = builtin_display() && display_services();
    if (has_builtin && index == 0) {
        int (*get)(CGDirectDisplayID, float *) = dlsym(display_services(), "DisplayServicesGetBrightness");
        float v = 0;
        if (get && get(builtin_display(), &v) == 0) return (int)(v * 100 + 0.5f);
        return -1;
    }
    AVService avs[8];
    int ext = external_services(avs, 8), at = index - has_builtin, out = -1;
    if (at >= 0 && at < ext) out = ddc_get(avs[at]);
    for (int i = 0; i < ext; i++) CFRelease(avs[i]);
    return out;
}

/// Set monitor `index` (-1 = every one) to 0..100; 0 when at least one took it.
int mh_monitor_set(int index, int value) {
    int done = 0, has_builtin = builtin_display() && display_services();
    if (has_builtin && (index == -1 || index == 0)) {
        int (*set)(CGDirectDisplayID, float) = dlsym(display_services(), "DisplayServicesSetBrightness");
        if (set && set(builtin_display(), value / 100.0f) == 0) done = 1;
    }
    AVService avs[8];
    int ext = external_services(avs, 8);
    for (int i = 0; i < ext; i++) {
        if (index == -1 || index - has_builtin == i) {
            const uint8_t send[] = {0x10, (uint8_t)(value >> 8), (uint8_t)(value & 0xff)};
            if (ddc_write(avs[i], send, 3) == 0) done = 1;
        }
        CFRelease(avs[i]);
    }
    return done ? 0 : -1;
}

// ---------- session lock and idle time ----------

/// 1 when the screen is locked, 0 when not, -1 when unknown (CGSessionCopyCurrentDictionary's
/// "CGSSessionScreenIsLocked", present only while locked).
int mh_session_locked(void) {
    CFDictionaryRef session = CGSessionCopyCurrentDictionary();
    if (!session) return -1;
    CFBooleanRef locked = CFDictionaryGetValue(session, CFSTR("CGSSessionScreenIsLocked"));
    int out = locked && CFGetTypeID(locked) == CFBooleanGetTypeID() && CFBooleanGetValue(locked) ? 1 : 0;
    CFRelease(session);
    return out;
}

/// Seconds since the last keyboard or mouse input in this session.
double mh_idle_seconds(void) {
    return CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType);
}
