"""Compare separate level/up/down F8 captures from one APB process launch."""

import argparse
import json
import math
import struct
from pathlib import Path

from apb_f8_common import utc_now, write_json


def load_capture(directory):
    report = json.loads((directory / "capture.json").read_text(encoding="utf-8"))
    if report.get("format") != "apb-camera-single-f8-v1":
        raise RuntimeError(f"Unsupported camera capture in {directory}")
    path = Path(report["matrix_blocks"]["path"])
    if path.stat().st_size != report["matrix_blocks"]["records"] * report["matrix_blocks"]["record_bytes"]:
        raise RuntimeError(f"Matrix-block file size mismatch: {path}")
    samples = {}
    with path.open("rb") as stream:
        while raw := stream.read(0x60):
            if len(raw) != 0x60:
                raise RuntimeError(f"Truncated matrix block in {path}")
            index, address, marker = struct.unpack_from("<IQI", raw)
            matrix = struct.unpack_from("<16f", raw, 0x10)
            position = struct.unpack_from("<3f", raw, 0x50)
            if address not in samples:
                samples[address] = {"index": index, "marker": marker,
                                    "matrix": matrix, "position": position}
    return report, samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("level", type=Path)
    parser.add_argument("up", type=Path)
    parser.add_argument("down", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Comparison output must be a new path")
    captures = [load_capture(path) for path in (args.level, args.up, args.down)]
    reports = [item[0] for item in captures]
    if any(report["identity"] != reports[0]["identity"] for report in reports[1:]):
        raise RuntimeError("Level/up/down captures belong to different APB process launches")
    if any(report["module_base"] != reports[0]["module_base"] for report in reports[1:]):
        raise RuntimeError("Level/up/down captures have different module bases")
    common = set(captures[0][1]) & set(captures[1][1]) & set(captures[2][1])
    candidates = []
    for address in common:
        level, up, down = (item[1][address] for item in captures)
        values = (*level["matrix"], *up["matrix"], *down["matrix"],
                  *level["position"], *up["position"], *down["position"])
        if not all(math.isfinite(value) for value in values):
            continue
        up_vertical = up["matrix"][11]
        down_vertical = down["matrix"][11]
        if up_vertical <= 0.5 or down_vertical >= -0.5:
            continue
        candidates.append({
            "address": f"0x{address:X}", "marker": level["marker"],
            "table_indices": [level["index"], up["index"], down["index"]],
            "vertical_component": [level["matrix"][11], up_vertical, down_vertical],
            "camera_positions": [level["position"], up["position"], down["position"]],
            "max_matrix_change_up_down": max(abs(a - b) for a, b in zip(up["matrix"], down["matrix"])),
        })
    candidates.sort(key=lambda item: (-item["max_matrix_change_up_down"], item["address"]))
    result = {
        "format": "apb-camera-pitch-leads-v1", "created_utc": utc_now(),
        "identity": reports[0]["identity"], "module_base": reports[0]["module_base"],
        "sources": {name: str(path) for name, path in zip(("level", "up", "down"),
                                                       (args.level, args.up, args.down))},
        "shared_object_addresses": len(common), "status": "unclassified_pitch_leads",
        "candidate_count": len(candidates), "candidates": candidates,
    }
    write_json(args.output, result)
    print(f"Compared {len(common)} shared objects; found {len(candidates)} pitch-pattern leads.")
    for item in candidates[:25]:
        print(f"  {item['address']} marker={item['marker']} "
              f"vertical={tuple(round(value, 3) for value in item['vertical_component'])} "
              f"matrix delta={item['max_matrix_change_up_down']:.3f}")
    print(f"Saved: {args.output}")
    if not candidates:
        raise RuntimeError("No pitch-pattern camera lead in the sampled table; inspect raw captures before widening the search")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera comparison failed: {error}")
