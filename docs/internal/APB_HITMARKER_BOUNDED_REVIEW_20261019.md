# Bounded hitmarker evidence review

Read-only offline analysis of `captures/hitmarker-20620-20261019-overlay-r2/events.jsonl` paired by image filename with `image-classification.json`. No new scan, input, build, or automated test was performed.

## Sample

274 paired samples: 90 idle, 89 red-target phase, 95 white-control phase. Image classification identified eight red hitmarker frames, all in the red-target phase; phase membership alone is not a hit label. Positive red ticks: 18, 19, 57, 62, 63, 80, 81, 82.

Bounded sampled bodies: HUD 0x1000 bytes, controller 0x1400, pawn 0x800. These bounds do not establish verified allocation sizes or exhaust all linked objects.

## Results

- No HUD/pawn byte or aligned u32 value matched at least five positive frames with twelve or fewer negative matches.
- No HUD/pawn individual bit matched at least six positives with twenty or fewer negative matches.
- No aligned f32 in any of the three bodies had World-clock age in [0,1] seconds in at least five positives with thirty-five or fewer negative matches. This does not exclude different clock epochs, durations, doubles, linked event objects, or transient events between samples.
- Controller words at +0xFE0, +0x1284 and +0x12A8 behaved identically under the strongest weak predicate: byte at word+3 has bit 0x08 clear. That predicate matches seven of eight positive frames and fifteen non-hitmarker frames. All 22 matches occur during red-target phase; none occur in idle or white controls. It misses one labelled hitmarker and has substantial false positives. No gameplay meaning is established; do not use it as a trigger.
- Controller +0x484 is an integer World-seconds field: examples 942 at World 942.25, 954 at 954.10, 959 at 959.50, 973 at 973.77. Apparent association with the sequential red phase is elapsed-time confounding, not hit evidence.

## Decision

No direct hitmarker memory offset is verified within these captured body bounds. The duplicate controller predicate is at most a lead for further controlled recording; it is not a reliable hit event or damage attribution field. No production trigger should be added from this result. Screen-based detection remains a separate explicitly approved implementation choice, not an implicit fallback for absent memory evidence.


## Selected implementation constraint

The user explicitly rejected pixel-based hitmarker detection on 2026-10-19. Only a verified direct memory event, comparable to the existing cursor memory reader, is acceptable for hit sounds. The captured image classification is diagnostic ground truth only; do not turn it into a production detector or automatic fallback. Verux WAV playback remains pending a memory signal that distinguishes red/flesh hits from white hits.
