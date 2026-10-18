# Tagged pumpkin state audit — 2026-10-18

Evidence: `captures/pumpkin-nearby-20261018-a69f2f6d-r3/profile.json`, its `annotations.jsonl`, and `captures/pumpkin-tagged-state-audit.json`. The last file contains current bounded reads, linked-object probes, exact process identity, and read time. These are discovery observations for the explicitly identified build, not production offsets.

The operator tagged eight objects representing four pairs and confirmed that the first pumpkin was shot after tagging. The last pair was tagged at an already-smashed pumpkin location. Other matching pairs included ammunition distributors: the emitter-to-prefab association is not a pumpkin classifier.

| Pair order | Prefab | Emitter | Operator observation |
|---|---|---|---|
| 1 | `0x18266A000` | `0x18C297580` | Shot after tagging |
| 2 | `0x18250A400` | `0x18C08BC80` | Tagged pumpkin |
| 3 | `0x18250A000` | `0x18C286E80` | Tagged pumpkin |
| 4 | `0x18250B000` | `0x24F31E780` | Already smashed before tagging |

## Observations

- Initial scan versus annotation: no differences in the saved first `0x100` bytes of any of the eight objects.
- Initial scan versus the later live audit: all eight fingerprints remained identical and the first `0x300` bytes were unchanged. This includes the first, subsequently-shot pair and the final, already-smashed pair.
- The typed emitter-to-prefab link and location survived. This supports projecting the location after smashing; their continued existence does not establish availability.
- At audit time, actor-array membership was stable during each pair's local inspection. Only the prefab and emitter were found within 150 world units of each tagged prefab in that level's actor array. No earlier nearby-actor snapshot was retained for these locations, so this cannot establish actor removal on destruction.
- Linked objects reached through the probed `+0x128` and `+0x204` arrays were shared across all four pairs. Their presence alone provides no observed distinction between intact and smashed pumpkins.

## Limits

The annotation overlay did not retain a continuous before/after stream around the shot. The later bounded `0x800` reads are not verified allocation sizes; only the first `0x300` bytes have the initial raw baseline. No availability flag or pumpkin-specific identity discriminator was confirmed. A later capture needs to follow the destructible gameplay actor or component through a controlled shot; these persistent location objects alone are insufficient.
