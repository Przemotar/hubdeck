// Lights to the music on macOS (owner's "1 a", 2026-10-30: "Chyba nie działa światło pod muzykę";
// it was Windows only). What plays is heard through a Core Audio process tap of every process,
// not muted (macOS 14.2+, the API the per-app volume uses), as Windows hears the speakers'
// loopback (engine/beat.rs). The tap's IO block mixes each buffer to mono into a ring that
// beat.rs reads every 10 ms. The first tap asks the user once (System Audio Recording).
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>
#import <CoreAudio/CoreAudio.h>
#import <Foundation/Foundation.h>
#include <stdatomic.h>

// mono samples kept for the reader: ~2.7 s at 48 kHz, far more than 10 ms between its reads
#define RING (1u << 17)
static float ring[RING];
static _Atomic uint64_t written;
static uint64_t read_at;
static AudioObjectID tap_id, aggregate_id;
static AudioDeviceIOProcID proc_id;
static NSString *output_uid;

static AudioObjectPropertyAddress global(AudioObjectPropertySelector selector) {
    return (AudioObjectPropertyAddress){selector, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
}

static NSString *default_output(void) {
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

static void stop(void) API_AVAILABLE(macos(14.2)) {
    if (aggregate_id) {
        if (proc_id) {
            AudioDeviceStop(aggregate_id, proc_id);
            AudioDeviceDestroyIOProcID(aggregate_id, proc_id);
        }
        AudioHardwareDestroyAggregateDevice(aggregate_id);
    }
    if (tap_id) AudioHardwareDestroyProcessTap(tap_id);
    tap_id = aggregate_id = 0;
    proc_id = NULL;
    output_uid = nil;
}

// 0 listening, with the rate and channels written; 2 macOS older than 14.2, 3 the tap was refused
// (most often: System Audio Recording not allowed), 4 no output, 5 the device was refused, 6 it
// would not start, 7 a sound format that is not float.
static int start(double *rate, int *channels) API_AVAILABLE(macos(14.2)) {
    stop();
    NSString *output = default_output();
    if (!output) return 4;
    // the class by name: a direct reference would stop HubDeck from starting on macOS 12 and 13
    Class tapClass = NSClassFromString(@"CATapDescription");
    if (!tapClass) return 2;
    CATapDescription *d = [[tapClass alloc] initStereoGlobalTapButExcludeProcesses:@[]];
    d.name = @"HubDeck lights to the music";
    d.privateTap = YES;
    d.muteBehavior = CATapUnmuted;
    if (AudioHardwareCreateProcessTap(d, &tap_id) != noErr || !tap_id) {
        tap_id = 0;
        return 3;
    }
    AudioStreamBasicDescription f = {0};
    UInt32 size = sizeof f;
    AudioObjectPropertyAddress fa = global(kAudioTapPropertyFormat);
    if (AudioObjectGetPropertyData(tap_id, &fa, 0, NULL, &size, &f) != noErr || f.mFormatID != kAudioFormatLinearPCM || !(f.mFormatFlags & kAudioFormatFlagIsFloat) || f.mBitsPerChannel != 32) {
        stop();
        return 7;
    }
    NSDictionary *agg = @{
        @kAudioAggregateDeviceNameKey : @"HubDeck lights to the music",
        @kAudioAggregateDeviceUIDKey : [NSUUID UUID].UUIDString,
        @kAudioAggregateDeviceMainSubDeviceKey : output,
        @kAudioAggregateDeviceIsPrivateKey : @YES,
        @kAudioAggregateDeviceIsStackedKey : @NO,
        @kAudioAggregateDeviceTapAutoStartKey : @YES,
        @kAudioAggregateDeviceSubDeviceListKey : @[ @{@kAudioSubDeviceUIDKey : output} ],
        @kAudioAggregateDeviceTapListKey : @[ @{@kAudioSubTapDriftCompensationKey : @YES, @kAudioSubTapUIDKey : d.UUID.UUIDString} ],
    };
    if (AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)agg, &aggregate_id) != noErr) {
        aggregate_id = 0;
        stop();
        return 5;
    }
    AudioDeviceIOBlock block = ^(const AudioTimeStamp *now, const AudioBufferList *in, const AudioTimeStamp *inTime, AudioBufferList *out, const AudioTimeStamp *outTime) {
        (void)now; (void)inTime; (void)outTime;
        // the aggregate's own output stays silent: the music already plays straight
        for (UInt32 i = 0; i < out->mNumberBuffers; i++) {
            if (out->mBuffers[i].mData) memset(out->mBuffers[i].mData, 0, out->mBuffers[i].mDataByteSize);
        }
        if (in->mNumberBuffers == 0) return;
        // interleaved (one buffer, n channels) or one buffer a channel: the mean of all
        UInt32 per = in->mBuffers[0].mNumberChannels ? in->mBuffers[0].mNumberChannels : 1;
        UInt32 frames = in->mBuffers[0].mDataByteSize / (sizeof(float) * per);
        UInt32 total = 0;
        for (UInt32 b = 0; b < in->mNumberBuffers; b++) total += in->mBuffers[b].mNumberChannels ? in->mBuffers[b].mNumberChannels : 1;
        uint64_t w = atomic_load_explicit(&written, memory_order_relaxed);
        for (UInt32 k = 0; k < frames; k++) {
            float sum = 0;
            for (UInt32 b = 0; b < in->mNumberBuffers; b++) {
                const float *s = in->mBuffers[b].mData;
                UInt32 n = in->mBuffers[b].mNumberChannels ? in->mBuffers[b].mNumberChannels : 1;
                if (!s || (k + 1) * n * sizeof(float) > in->mBuffers[b].mDataByteSize) continue;
                for (UInt32 c = 0; c < n; c++) sum += s[k * n + c];
            }
            ring[(w + k) % RING] = sum / (float)total;
        }
        atomic_store_explicit(&written, w + frames, memory_order_release);
    };
    if (AudioDeviceCreateIOProcIDWithBlock(&proc_id, aggregate_id, NULL, block) != noErr || AudioDeviceStart(aggregate_id, proc_id) != noErr) {
        stop();
        return 6;
    }
    read_at = atomic_load_explicit(&written, memory_order_acquire);
    output_uid = output;
    *rate = f.mSampleRate;
    *channels = (int)f.mChannelsPerFrame;
    return 0;
}

// Called from a Rust thread with no autorelease pool: each call drains its own.
int mh_listen_start(double *rate, int *channels) {
    @autoreleasepool {
        if (@available(macOS 14.2, *)) return start(rate, channels);
        return 2;
    }
}

void mh_listen_stop(void) {
    @autoreleasepool {
        if (@available(macOS 14.2, *)) stop();
    }
}

// The mono samples heard since the last read, at most `max` (the newest when more came).
uint32_t mh_listen_read(float *out, uint32_t max) {
    uint64_t w = atomic_load_explicit(&written, memory_order_acquire);
    if (w - read_at > RING) read_at = w - RING;
    uint64_t n = w - read_at;
    if (n > max) {
        read_at = w - max;
        n = max;
    }
    for (uint64_t i = 0; i < n; i++) out[i] = ring[(read_at + i) % RING];
    read_at += n;
    return (uint32_t)n;
}

// 1 when the default output is not the one listened to (headphones in or out): start again.
int mh_listen_output_changed(void) {
    @autoreleasepool {
        NSString *now = default_output();
        return output_uid && now && ![now isEqualToString:output_uid];
    }
}
