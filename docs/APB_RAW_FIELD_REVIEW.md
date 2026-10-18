# Saved APB raw-field review — 2026-10-01

This is an offline review of the September 30 capture collection. No live
process was inspected and no runtime behavior or eligibility rule was changed.
All numeric locations below are tied to executable SHA-256
`3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640`.

The review records SHA-256 provenance for 2,330 inputs and examines 462,415
survey body observations, 36 character observations from six identity captures,
14 mission annotations plus their local contexts, and four vehicle stages.
These are repeated observations, not player counts. Exact duplicate candidate/
linked bodies are merged within a dataset and survey; differing sequential reads
are retained. Capture membership is not a complete or independently verified
district roster. Probe lengths are not verified allocation sizes.

## +0x84 depends on the object type

The original `1/9 -> 4` observation belongs to vtable RVA `0x2AE68D8`, marker
294, whose recorded XYZ starts at `+0xD0`. It does not belong to the validated
character pawn, vtable RVA `0x2AAEFA0`, marker 5778. Identical offsets in different
types need not have identical meanings.

### Original position-object type: marker 294

| Unsigned 32-bit value | Hex | Set bits | Survey observations |
|---:|---|---|---:|
| 0 | 0x0 | none | 76,357 |
| 1 | 0x1 | 0x1 | 50,286 |
| 4 | 0x4 | 0x4 | 915 |
| 5 | 0x5 | 0x1, 0x4 | 1 |
| 8 | 0x8 | 0x8 | 175 |
| 9 | 0x9 | 0x1, 0x8 | 266 |
| 12 | 0xC | 0x4, 0x8 | 684 |

This fits combinations of three bits better than an exclusive state number,
but no engine flag names have been resolved. The sole value-5 observation is
address `0x3918DDD00`, `social-manual-labels-20260930-1/survey-0067`.
Examples for each value, their source files, process identities and instance
fingerprints are preserved in the evidence JSON.

The previously discussed address `0x2BB04E400` has this saved sequence:

| Survey | +0x84 | +0xF8 and +0x420 |
|---|---:|---|
| 0–1 | 1 | both point to 0x2BB065600 |
| 2 | 9 | both point to 0x2BB065600 |
| 3 | 1 | both point to 0x2BB065600 |
| 4 | 9 | both point to 0x2BB065600 |
| 5–9 | 1 | both point to 0x2BB065600 |
| 10–18 | 4 | both NULL |

Operator-labelled dead stage is survey 13; respawned stage is survey 18. Thus
the value-4 change occurred before the dead-stage marker, and the old object
remained at 4 through the respawned marker. This does not establish a death
flag, a current respawned character, or persistent human identity.

### Character pawn: marker 5778

| +0x84 | Decimal | Bits added to baseline 0x4088 | Survey observations |
|---|---:|---|---:|
| 0x4088 | 16,520 | none | 936 |
| 0x4488 | 17,544 | 0x400 | 251 |
| 0xC088 | 49,288 | 0x8000 | 225 |
| 0xC488 | 50,312 | 0x8000, 0x400 | 175 |
| 0xC0A8 | 49,320 | 0x8000, 0x20 | 53 |

All 36 character observations across the six identity inspections were
`0x4088`. The broader surveys expose additional values.

The controlled local character `0x39216F040` has:

| Stage | +0x84 | Raw +0x178 |
|---|---|---|
| On foot | 0x4088 | nonvehicle attachment |
| Driving stationary | 0x4488 | possessed vehicle 0x77F26200 |
| Driving moved | 0x4488 | possessed vehicle 0x77F26200 |
| Exited | 0x4088 | nonvehicle attachment |

The changed bit is exactly `0x400`. This is one controlled association, not
a resolved field name or a universal vehicle detector. Mission annotations
0008/0009 have vehicle attachments while remaining `0x4088`.

The other labelled character values are useful counterexamples:

| Pawn address | Recorded role | Annotation | +0x84 |
|---|---|---|---|
| 0x17212B040 | opposition | 0000, 0002 | 0x4088 |
| 0x16DD110C0 | opposition | 0001 | 0x4088 |
| 0x19DDEC100 | opposition | 0003 | 0x4088 |
| 0x1967E0100 | teammate | 0004 | 0x4088 |
| 0x179E910C0 | teammate | 0005 | 0x4088 |
| 0x16E9850C0 | opposition | 0006 | 0x4088 |
| 0x1966390C0 | opposition, shorter collision bounds | 0007 | 0x4088 |
| 0x171ED3040 | outside mission, vehicle-attached | 0008, 0009 | 0x4088 |
| 0x179EAC100 | opposition, vehicle-attached | 0010 | 0x4488 |
| 0x1923E10C0 | teammate, vehicle-attached | 0011 | 0x4488 |
| 0x16EAB90C0 | opposition, per user correction | 0012, 0013 | 0x4088 |

So this field alone does not distinguish mission side, vehicle attachment or
the shorter stance observed in annotation 0007.

For completeness, position component type `(0x3142FB0, 4741)` produced
`0x88`, `0x4088`, `0x4140`, `0x4148`, and `0x8088`. Linked lifecycle type
`(0x2AFF240, 1062)` produced `0`, `1`, `4`, `5`, `8`, and `9`.
These distributions remain separate in the machine-readable evidence.

## Other numeric leads

All offsets here are from the character pawn and all field names remain
unresolved except the independently validated position and bounds routes.

| Offset / decoding | Observed values | Interpretation limit |
|---|---|---|
| +0x328, float | 79 in 13 labelled events, 48 in annotation 0007 | Eye-height/stance lead; same event has collision half-height 60 |
| +0x374, float | 0 normally in labelled events, -31 in annotation 0007 | Related scalar candidate; controlled local vehicle sequence instead has approximately 83.664 here |
| +0x448, float | 79 normally, 110 in annotation 0007 | Complementary change of +31; not a resolved height name |
| +0x6B4/+0x6B8/+0x6BC, floats | e.g. (6,64,0), (-554,-225,278.9754), (-557,224,0), and (0,0,0) | Vector-shaped movement lead; no measured velocity calibration |
| +0xE08 and +0xEB8, floats | both contain 0, 85 or 95; match in all 14 labelled events and four vehicle stages | Duplicate scalar candidate; no evidence these are health or FOV |
| +0x3E8, u32 | 200 | Constant in the 36 identity, 14 labelled and four controlled vehicle observations |
| +0x3EC, u32 | 3072 | Same scope; no resolved meaning |
| +0x424, u32 | 235 | Same scope; no resolved meaning |
| +0x434, u32 | 2000 | Same scope; no resolved meaning |
| +0x4B4, u32 | 100 | Same scope; could be configuration/default data rather than current state |
| +0x514, u32 | 1000 | Same scope; no damage/healing correlation to establish health |

The 100 and 1000 observations are numeric leads, not confirmed health values.
The review does not contain a controlled damage, recovery or stun sequence.
The vector triplet is not assumed to remain populated while driving: it was
zero in all four controlled vehicle character bodies, despite vehicle movement.
No bone array, bone names or skeletal transforms were validated.

## Strings

The decoder searches ASCII and ASCII-range UTF-16LE runs and separately
reviews saved possible-FString metadata. It yields 1,472 candidates, including
duplicates and coincidental printable bytes. This is not a string or name table.

- Name-like UTF-16 text `aBobra` at probe offsets `+0x18E8/+0x1908/+0x1928`
  and `gElite` at `+0x1D08` repeats near pawn `0x236597040` in identity captures
  3–5. Those offsets exceed the first 0x1000 bytes and may be neighboring UI
  allocations; they do not identify this pawn's player.
- Pointer-string metadata includes `Activator`, `Player Index`, `Gamepad Id`,
  `BabyShabooboo`, `Now viewing from`, and `Malzel`. These are readable captures,
  but their containing probe extents and owner relationships are not established.
- Captured pointer contexts also contain asset-path text ending in
  `v_a_classicmuscle_Exhausts_2.upk`. This is an asset-text lead, not a verified
  model label for the associated character or vehicle.
- Pawn +0x128's old possible-FString result was a two-character Unicode
  interpretation of array bytes. Earlier membership review found three shared
  objects there; that printable decode does not establish a string field.
- The only ASCII-pattern match in the first 0x1000 bytes of the six identity
  character captures is `G!FcD ` at +0x8F, crossing XYZ float bytes. It is a
  clear example of why printable bytes alone are insufficient evidence.

Numeric +0x84 values can be formatted as hex and lists of set bits. Mapping
them to words such as alive, hidden or crouched requires independent semantics;
the bytes do not supply those labels.

There are 291 recorded pointer-context read failures in the 14 annotations.
The original errors and addresses are retained in `context_capture_failures`.
Those contexts contribute no decoded strings; many candidate pointers are
numeric field pairs misread as addresses. This review made no new live reads
and did not switch memory readers.

## Reproduce

From the repository root, choose a new output path:

```powershell
python scripts/apb_raw_field_review.py --output "$env:TEMP\apb-raw-field-review.json"
```

Use `--root` to identify a relocated saved capture collection. The standard-
library script checks executable identities and supported types, validates
saved identity-object fingerprints and string descriptors, records input hashes,
and refuses to overwrite evidence. Missing required captures or malformed
records fail visibly. No new automated tests or test dependencies were added.

Saved evidence: [APB_RAW_FIELD_EVIDENCE.json](APB_RAW_FIELD_EVIDENCE.json).
