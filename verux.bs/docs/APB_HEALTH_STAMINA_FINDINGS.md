# Health/stamina scan findings — 2026-10-02

The run `health-stamina-laptop-20261002-162349` completed all eleven phases,
with three samples per phase, and exited 0. Its primary laptop pawn remained
`0x1F6C97040`, marker 5778, in the same APB process (PID 276 and recorded
creation time). No runtime health/stamina offset is established by this run.

The operator confirmed that stamina phases used **PIG less-lethal damage**,
and health visibly fell by about **85% (15% remained)**. That numeric report
does not specify an exact value for every phase. Original capture instructions
incorrectly described sprinting for stamina. Those instructions are retained
in the capture; future prompts are corrected. The operator's clarification
takes precedence when interpreting what was actually done.
The developer's [weapon-balance explanation](https://www.gamersfirst.com/apb/news/2020/9/11/weapon-balance)
also describes PIG stamina damage; its historical numeric damage values are
not assumed to apply to this captured build.

## Why the three automatic matches are not accepted resource values

All three matches came through **Pawn +0x35C -> 0x1F6C90F00**, vtable RVA
`0x2AFF240`, marker 1062. The component's +0x44 and +0x7C point back to the
selected pawn. Its first-sample bodies retain the same instance across phases.

| Automatic match | Interpretation from surrounding bytes | Disposition |
|---|---|---|
| Component +0x110, float32 | Z of the three-float origin at +0x108, whose X/Y closely match pawn world X/Y | Bounds-origin Z, not accepted as stamina |
| Component +0x112, unsigned byte | Byte 2 of that same +0x110 float | Duplicate partial-byte interpretation, not an independent resource |
| Component +0x386, unsigned 16-bit | Upper half of a float at +0x384, within the float triplet +0x37C/+0x380/+0x384 | Pose-associated/vector lead; no resource meaning established |

The component +0x114/+0x118/+0x11C floats form positive XYZ extents. Its +0x120
float equals their vector length within 0.000004 in every first-sample phase.
Together with the world-like origin and reciprocal owner links, this is strong
component-bounds evidence. The exact reflected component class and mesh/bone
semantics remain unresolved.

| Laptop phase | Pawn Z | Pawn +0x328 float | Component origin Z | Component Z half-extent |
|---|---:|---:|---:|---:|
| Full baseline | -456 | 79 | -467.044 | 85.967 |
| Health damaged | -456 | 79 | -467.044 | 85.966 |
| More health damage | -456 | 79 | -467.288 | 85.703 |
| Health recovered | -456 | 79 | -467.383 | 85.611 |
| Partial stamina damage | -456 | 79 | -472.154 | 80.916 |
| Low stamina | -487 | 48 | -505.835 | 47.486 |
| Stamina recovered | -456 | 79 | -467.028 | 85.982 |
| Repeated health damage | -456 | 79 | -467.422 | 85.568 |
| Health recovered again | -456 | 79 | -466.973 | 86.036 |
| Repeated stamina damage | -456 | 79 | -473.377 | 79.694 |
| Stamina recovered again | -456 | 79 | -467.054 | 85.950 |

The desktop screenshots show the selected character standing at baseline and
kneeling in the low-stamina frame, with a changed overhead icon. This explains
why geometry correlates with the operator-labelled stamina sequence. It does
not turn geometry into a stamina percentage. The repeat-low phase does not
have the same pawn Z or height as the first low phase.

The +0x384 float behind the unsigned-16 match changes from approximately
-1.678 at baseline to -58.798 at low stamina, then -1.647 after recovery.
Its adjacent floats change too. There is no independent numeric calibration
to health or stamina. The three automatic results are preserved unchanged in
`candidates.json`; this review rejects their interpretation as resource bars.

## Health and old raw leads

- Pawn +0x4B4 stayed at integer 100 in every phase.
- Pawn +0x514 stayed at integer 1000 in every phase.
- Pawn +0x84 stayed at 0x4088 in every phase, including low stamina/kneeling.
- No scalar met the original health pattern with recovery and stamina controls.

Given the operator's confirmed substantial health loss, 100 and 1000 cannot
be treated as responsive current health from this sequence. They could be
defaults, maxima or unrelated constants; this run does not identify their names.

A separate exploratory inspection of damage-versus-recovery separation,
without the original stamina-control requirement, yielded only two overlapping
interpretations of +0x3B4 on the object reached through Pawn +0x350. That field
is the next-pawn list link, not evidence of a health owner. The float also rises
most strongly during repeated stamina damage, so it is not accepted as health.
This exploratory inspection does not replace or weaken the original scan result.

## Scope and next useful evidence

The automatic reviewer reported 87 common subject slots, one of which was
excluded because its instance changed. Slots can refer to the same underlying
object; these are not 87 independent actors. Raw body hashes, lengths, types
and fingerprints are checked against capture metadata.

There were 1,962 failed speculative pointer probes across the 33 samples.
They are preserved with field locations and errors. Many originate from numeric
field pairs that merely resemble addresses. Essential pawn and phase captures
completed; unknown bytes behind failed probes remain unexamined.
Each complete phase took approximately 0.46–0.71 seconds, including screenshot
capture. The review uses the first sample and preserves subsequent burst values.

This is a bounded remote-client scan: first 0x3000 pawn bytes and first 0x1000
bytes of selected direct linked objects. It does not establish that health or
stamina is absent everywhere in desktop memory. Values may be indirect, encoded,
outside these extents, or unavailable for a remotely owned player.

The most discriminating next comparison is the laptop's locally owned character
under the same damage/recovery states. A local-client match would help separate
missing remote replication from a missed layout. No new capture or deployment
to the laptop was started by this review.

An additional drawing lead is the reciprocal component's dynamic bounds: its
Z half-extent changes from about 86 to 47.5 with the low-stamina pose. That is
potentially useful for pose-sensitive boxes; it is not verified bone data and
has not been added to product rendering.

## Reproduce the original candidate calculation

Choose a new output report:

```powershell
python scripts/apb_health_stamina_review.py '..\drayebr\build-rpm\snapshots\health-stamina-laptop-20261002-162349' '<new-review.json>'
```

The capture's `candidates.json` includes input SHA-256 records and raw phase
vectors. Source files, phase labels and automatic results were preserved.
No tests, fixtures, dependencies or runtime resource readers were added.
