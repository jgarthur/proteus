#!/usr/bin/env python3
"""Inspect sampled genome changes and creation ancestry from a proteus-audit run."""

import argparse
import json
from pathlib import Path


def rows(path):
    """Stream JSONL records without retaining unrelated genome data."""
    with path.open() as stream:
        for line in stream:
            yield json.loads(line)


def inspect(directory, uid):
    """Resolve a program's creation ancestry, sampled genomes and retained events."""
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest["audit_schema_version"] != "1.0.0":
        raise ValueError("unsupported audit schema")
    summary = json.loads((directory / "summary.json").read_text())
    if uid is None:
        return summary
    births = {row["uid"]: row for row in rows(directory / "births.jsonl")}
    identities = {key: value["parent_uid"] for key, value in births.items()}
    observations = []
    for row in rows(directory / "lineage.jsonl"):
        identities.setdefault(row["uid"], row["parent_uid"])
        if row["uid"] == uid:
            observations.append(row)
    lineage = []
    cursor = uid
    seen = set()
    while cursor:
        if cursor in seen:
            raise ValueError("cycle in creation ancestry")
        seen.add(cursor)
        parent = identities.get(cursor)
        lineage.append({"uid": cursor, "parent_uid": parent,
                        "birth": births.get(cursor), "identity_observed": cursor in identities})
        if parent is None:
            break
        cursor = parent
    wanted = {row["genome"] for row in observations}
    genomes = {row["genome"]: row["code"] for row in rows(directory / "genomes.jsonl")
               if row["genome"] in wanted}
    if wanted - genomes.keys():
        raise ValueError("sampled genome missing from dictionary")
    events = [row for row in rows(directory / "events.jsonl")
              if row["uid"] == uid or row["actor"] == uid or uid in (row.get("boot_actors") or [])]
    attempts = [row for row in rows(directory / "actions.jsonl") if row["actor"] == uid]
    return {"uid": uid, "creation_ancestry": lineage, "retained_attempts": attempts, "samples": observations,
            "genomes": genomes, "retained_events": events,
            "trace_interval": [manifest["trace_start"], manifest["trace_end"]]}


def main():
    """Print deterministic JSON for a run summary or one program's history."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("audit", type=Path)
    parser.add_argument("--uid", type=int)
    args = parser.parse_args()
    if args.uid is not None and args.uid <= 0:
        parser.error("uid must be positive")
    print(json.dumps(inspect(args.audit, args.uid), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
