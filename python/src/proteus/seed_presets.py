from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class SeedPreset:
    key: str
    label: str
    description: str
    assembly_source: str


BASIC_SEED = """absorb
takeM
cw
push 0
setSrc
getSize
for
read
appendAdj
next
boot
"""


MOVER_SEED = """absorb
takeM
move
cw
push 0
setSrc
getSize
for
read
appendAdj
next
boot
"""


GATED_SEED = """absorb
takeM
cw
getM
getSize
lt
push -8
swap
jmpNZ
getE
getSize
lt
push -8
push -8
add
push -2
add
swap
jmpNZ
push 0
setSrc
getSize
for
read
appendAdj
next
boot
"""


GATED_MOVER_SEED = """absorb
takeM
move
cw
getM
getSize
lt
push -8
push -3
add
swap
jmpNZ
getE
getSize
lt
push -8
push -8
add
push -5
add
swap
jmpNZ
push 0
setSrc
getSize
for
read
appendAdj
next
boot
"""


SEED_PRESETS: tuple[SeedPreset, ...] = (
    SeedPreset(
        key="basic",
        label="Basic",
        description="The baseline seed replicator from the original setup.",
        assembly_source=BASIC_SEED,
    ),
    SeedPreset(
        key="mover",
        label="Mover",
        description="Adds `move` immediately after `takeM` before starting replication.",
        assembly_source=MOVER_SEED,
    ),
    SeedPreset(
        key="gated",
        label="Gated",
        description="Waits to enter replication until local mass and energy meet the genome-size budget.",
        assembly_source=GATED_SEED,
    ),
    SeedPreset(
        key="gated_mover",
        label="Gated + mover",
        description="Uses the resource gate and also inserts `move` immediately after `takeM`.",
        assembly_source=GATED_MOVER_SEED,
    ),
)


def preset_map() -> dict[str, SeedPreset]:
    return {preset.key: preset for preset in SEED_PRESETS}


def preset_payload() -> list[dict[str, str]]:
    return [
        {
            "key": preset.key,
            "label": preset.label,
            "description": preset.description,
            "assembly_source": preset.assembly_source,
        }
        for preset in SEED_PRESETS
    ]
