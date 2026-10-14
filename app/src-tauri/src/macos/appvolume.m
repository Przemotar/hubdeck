// Per-app volume on macOS (owner, 2026-10-13: "rób wszystko teraz"). macOS has no API to set one
// app's volume; since macOS 14.2 an app's sound can be tapped (Core Audio process taps): HubDeck
// taps the app with "muted when tapped", so the app is no longer heard directly, and plays the
// tapped sound to the speakers itself at the volume asked for (an aggregate device of the default
// output and the tap, as SoundSource and open-source tools do). Back at full volume the tap goes
// and the app plays straight again. The first tap asks the user once (System Audio Recording).
#import <AppKit/AppKit.h>
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>
#import <CoreAudio/CoreAudio.h>
#import <Foundation/Foundation.h>
#include <stdatomic.h>

@interface MHTap : NSObject {
@public
    _Atomic float gain;
    AudioObjectID tap;
    AudioObjectID aggregate;
    AudioDeviceIOProcID proc;
}
@property(strong) NSArray<NSNumber *> *processes;
@end
@implementation MHTap
@end

static NSMutableDictionary<NSString *, MHTap *> *taps(void) {
    static NSMutableDictionary *all;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ all = [NSMutableDictionary new]; });
    return all;
}

static AudioObjectPropertyAddress global(AudioObjectPropertySelector selector) {
    return (AudioObjectPropertyAddress){selector, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
}

/// The audio process objects whose app matches any of `needles` (lowercase parts of a bundle id or
/// an app name: "spotify", "chrome"); helper processes count (a browser plays from its helpers).
API_AVAILABLE(macos(14.2))
static NSArray<NSNumber *> *processes_matching(NSArray<NSString *> *needles) {
    AudioObjectPropertyAddress a = global(kAudioHardwarePropertyProcessObjectList);
    UInt32 size = 0;
    if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &a, 0, NULL, &size) != noErr || size == 0) return @[];
    NSMutableData *data = [NSMutableData dataWithLength:size];
    if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, data.mutableBytes) != noErr) return @[];
    AudioObjectID *ids = data.mutableBytes;
    NSMutableArray *out = [NSMutableArray array];
    for (UInt32 i = 0; i < size / sizeof(AudioObjectID); i++) {
        NSMutableString *who = [NSMutableString string];
        CFStringRef bundle = NULL;
        UInt32 s = sizeof bundle;
        AudioObjectPropertyAddress b = global(kAudioProcessPropertyBundleID);
        if (AudioObjectGetPropertyData(ids[i], &b, 0, NULL, &s, &bundle) == noErr && bundle) {
            [who appendString:(__bridge NSString *)bundle];
            CFRelease(bundle);
        }
        pid_t pid = 0;
        s = sizeof pid;
        AudioObjectPropertyAddress p = global(kAudioProcessPropertyPID);
        if (AudioObjectGetPropertyData(ids[i], &p, 0, NULL, &s, &pid) == noErr) {
            NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
            if (app.localizedName) [who appendFormat:@" %@", app.localizedName];
        }
        NSString *lower = who.lowercaseString;
        for (NSString *needle in needles) {
            if (needle.length && [lower containsString:needle]) {
                [out addObject:@(ids[i])];
                break;
            }
        }
    }
    return out;
}

static NSString *default_output_uid(void) {
    AudioObjectID device = 0;
    UInt32 size = sizeof device;
    AudioObjectPropertyAddress a = global(kAudioHardwarePropertyDefaultOutputDevice);
    if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, &device) != noErr || !device) return nil;
    CFStringRef uid = NULL;
    size = sizeof uid;
    AudioObjectPropertyAddress u = global(kAudioDevicePropertyDeviceUID);
    if (AudioObjectGetPropertyData(device, &u, 0, NULL, &size, &uid) != noErr || !uid) return nil;
    return (__bridge_transfer NSString *)uid;
}

static void destroy(MHTap *t) API_AVAILABLE(macos(14.2)) {
    if (t->aggregate) {
        if (t->proc) {
            AudioDeviceStop(t->aggregate, t->proc);
            AudioDeviceDestroyIOProcID(t->aggregate, t->proc);
        }
        AudioHardwareDestroyAggregateDevice(t->aggregate);
    }
    if (t->tap) AudioHardwareDestroyProcessTap(t->tap);
    t->tap = t->aggregate = 0;
    t->proc = NULL;
}

/// A tap of these processes (or of everything but them when `others`), played at `gain`.
API_AVAILABLE(macos(14.2))
static MHTap *create(NSArray<NSNumber *> *processes, BOOL others, float gain, int *error) {
    NSString *output = default_output_uid();
    if (!output) {
        *error = 4;
        return nil;
    }
    // the class by name: a direct reference would stop HubDeck from starting on macOS 12 and 13
    Class tapClass = NSClassFromString(@"CATapDescription");
    if (!tapClass) {
        *error = 2;
        return nil;
    }
    CATapDescription *d = others ? [[tapClass alloc] initStereoGlobalTapButExcludeProcesses:processes] : [[tapClass alloc] initStereoMixdownOfProcesses:processes];
    d.name = @"HubDeck app volume";
    d.privateTap = YES;
    d.muteBehavior = CATapMutedWhenTapped;
    MHTap *t = [MHTap new];
    t.processes = processes;
    atomic_store(&t->gain, gain);
    if (AudioHardwareCreateProcessTap(d, &t->tap) != noErr || !t->tap) {
        // most often: System Audio Recording not allowed for HubDeck
        *error = 3;
        return nil;
    }
    NSDictionary *agg = @{
        @kAudioAggregateDeviceNameKey : @"HubDeck app volume",
        @kAudioAggregateDeviceUIDKey : [NSUUID UUID].UUIDString,
        @kAudioAggregateDeviceMainSubDeviceKey : output,
        @kAudioAggregateDeviceIsPrivateKey : @YES,
        @kAudioAggregateDeviceIsStackedKey : @NO,
        @kAudioAggregateDeviceTapAutoStartKey : @YES,
        @kAudioAggregateDeviceSubDeviceListKey : @[ @{@kAudioSubDeviceUIDKey : output} ],
        @kAudioAggregateDeviceTapListKey : @[ @{@kAudioSubTapDriftCompensationKey : @YES, @kAudioSubTapUIDKey : d.UUID.UUIDString} ],
    };
    if (AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)agg, &t->aggregate) != noErr) {
        destroy(t);
        *error = 5;
        return nil;
    }
    MHTap *__unsafe_unretained weak = t;
    AudioDeviceIOBlock block = ^(const AudioTimeStamp *now, const AudioBufferList *in, const AudioTimeStamp *inTime, AudioBufferList *out, const AudioTimeStamp *outTime) {
        float g = atomic_load(&weak->gain);
        for (UInt32 i = 0; i < out->mNumberBuffers; i++) {
            AudioBuffer *o = &out->mBuffers[i];
            UInt32 copied = 0;
            if (i < in->mNumberBuffers && in->mBuffers[i].mData && o->mData) {
                const AudioBuffer *src = &in->mBuffers[i];
                copied = MIN(src->mDataByteSize, o->mDataByteSize);
                const float *a = src->mData;
                float *b = o->mData;
                for (UInt32 k = 0; k < copied / sizeof(float); k++) b[k] = a[k] * g;
            }
            if (o->mData && o->mDataByteSize > copied) memset((char *)o->mData + copied, 0, o->mDataByteSize - copied);
        }
    };
    if (AudioDeviceCreateIOProcIDWithBlock(&t->proc, t->aggregate, NULL, block) != noErr || AudioDeviceStart(t->aggregate, t->proc) != noErr) {
        destroy(t);
        *error = 6;
        return nil;
    }
    return t;
}

/// Set the volume of the apps matching `needles` (newline-separated, lowercase), or of every
/// other app when `others`, to `gain` (0..1). `key` names the control's target. 0 on success;
/// 1 no such app plays, 2 macOS older than 14.2, 3 not allowed (System Audio Recording),
/// 4 no output, 5/6 the device would not start.
static int appvol_set(const char *key, const char *needles, int others, float gain) {
    if (@available(macOS 14.2, *)) {
        @synchronized(taps()) {
            NSString *k = [NSString stringWithUTF8String:key];
            NSArray *words = [[[NSString stringWithUTF8String:needles] lowercaseString] componentsSeparatedByString:@"\n"];
            NSArray *processes = processes_matching(words);
            MHTap *t = taps()[k];
            // full volume: the app plays straight again
            if (gain >= 0.999f) {
                if (t) {
                    destroy(t);
                    [taps() removeObjectForKey:k];
                }
                return 0;
            }
            if (!others && processes.count == 0) return 1;
            if (t && [t.processes isEqualToArray:processes]) {
                atomic_store(&t->gain, gain);
                return 0;
            }
            // new or restarted processes: a new tap
            if (t) {
                destroy(t);
                [taps() removeObjectForKey:k];
            }
            int error = 0;
            MHTap *made = create(processes, others != 0, gain, &error);
            if (!made) return error;
            taps()[k] = made;
            return 0;
        }
    }
    return 2;
}

/// The volume HubDeck plays `key` at (1 when it does not tap it).
static float appvol_get(const char *key) {
    @synchronized(taps()) {
        MHTap *t = taps()[[NSString stringWithUTF8String:key]];
        return t ? atomic_load(&t->gain) : 1.0f;
    }
}

/// Every app back to playing straight (HubDeck quits).
static void appvol_reset(void) {
    if (@available(macOS 14.2, *)) {
        @synchronized(taps()) {
            for (MHTap *t in taps().allValues) destroy(t);
            [taps() removeAllObjects];
        }
    }
}

// Called from Rust threads that have no autorelease pool: each call drains its own, or the
// autoreleased strings, arrays and app objects of every turn of a knob would stay until the
// thread ends (2026-10-14: the same gap grew HubDeck to gigabytes in the icon reader).
int mh_appvol_set(const char *key, const char *needles, int others, float gain) {
    @autoreleasepool {
        return appvol_set(key, needles, others, gain);
    }
}

float mh_appvol_get(const char *key) {
    @autoreleasepool {
        return appvol_get(key);
    }
}

void mh_appvol_reset(void) {
    @autoreleasepool {
        appvol_reset();
    }
}
