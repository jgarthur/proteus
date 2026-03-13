from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any


FIXTURE_DIR = Path(__file__).parent / "golden_archives"


def canonicalize_archive_snapshot(archive: dict[str, Any]) -> dict[str, Any]:
    snapshot = deepcopy(archive)
    snapshot.pop("reset_archive", None)
    snapshot["world"]["programs"] = sorted(
        snapshot["world"]["programs"],
        key=lambda item: item["program_id"],
    )
    snapshot["world"]["radiation"] = sorted(
        (
            {
                "x": entry["x"],
                "y": entry["y"],
                "packets": sorted(
                    entry.get("packets", []),
                    key=lambda packet: (
                        packet["x"],
                        packet["y"],
                        packet["direction"],
                        packet["value"],
                    ),
                ),
            }
            for entry in snapshot["world"].get("radiation", [])
        ),
        key=lambda entry: (entry["y"], entry["x"]),
    )
    return snapshot


def load_archive_fixture(name: str) -> dict[str, Any]:
    return json.loads((FIXTURE_DIR / f"{name}.json").read_text())
