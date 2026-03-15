from __future__ import annotations

from dataclasses import dataclass


IMMEDIATE_KIND = "immediate"
LOCAL_KIND = "local"
NONLOCAL_KIND = "nonlocal"


@dataclass(frozen=True, slots=True)
class InstructionDef:
    name: str
    opcode: int
    kind: str
    base_cost: int = 0
    opens_cell: bool = False
    requires_protection_break: bool = False


FIXED_INSTRUCTIONS: tuple[InstructionDef, ...] = (
    InstructionDef("dup", 0x10, IMMEDIATE_KIND),
    InstructionDef("drop", 0x11, IMMEDIATE_KIND),
    InstructionDef("swap", 0x12, IMMEDIATE_KIND),
    InstructionDef("over", 0x13, IMMEDIATE_KIND),
    InstructionDef("rand", 0x14, IMMEDIATE_KIND),
    InstructionDef("add", 0x20, IMMEDIATE_KIND),
    InstructionDef("sub", 0x21, IMMEDIATE_KIND),
    InstructionDef("neg", 0x22, IMMEDIATE_KIND),
    InstructionDef("eq", 0x23, IMMEDIATE_KIND),
    InstructionDef("lt", 0x24, IMMEDIATE_KIND),
    InstructionDef("gt", 0x25, IMMEDIATE_KIND),
    InstructionDef("not", 0x26, IMMEDIATE_KIND),
    InstructionDef("and", 0x27, IMMEDIATE_KIND),
    InstructionDef("or", 0x28, IMMEDIATE_KIND),
    InstructionDef("for", 0x30, IMMEDIATE_KIND),
    InstructionDef("next", 0x31, IMMEDIATE_KIND),
    InstructionDef("jmp", 0x32, IMMEDIATE_KIND),
    InstructionDef("jmpNZ", 0x33, IMMEDIATE_KIND),
    InstructionDef("jmpZ", 0x34, IMMEDIATE_KIND),
    InstructionDef("cw", 0x40, IMMEDIATE_KIND),
    InstructionDef("ccw", 0x41, IMMEDIATE_KIND),
    InstructionDef("getSize", 0x42, IMMEDIATE_KIND),
    InstructionDef("getIP", 0x43, IMMEDIATE_KIND),
    InstructionDef("getFlag", 0x44, IMMEDIATE_KIND),
    InstructionDef("getMsg", 0x45, IMMEDIATE_KIND),
    InstructionDef("getID", 0x46, IMMEDIATE_KIND),
    InstructionDef("getSrc", 0x47, IMMEDIATE_KIND),
    InstructionDef("getDst", 0x48, IMMEDIATE_KIND),
    InstructionDef("setDir", 0x49, IMMEDIATE_KIND),
    InstructionDef("setSrc", 0x4A, IMMEDIATE_KIND),
    InstructionDef("setDst", 0x4B, IMMEDIATE_KIND),
    InstructionDef("setID", 0x4C, IMMEDIATE_KIND),
    InstructionDef("getE", 0x4D, IMMEDIATE_KIND),
    InstructionDef("getM", 0x4E, IMMEDIATE_KIND),
    InstructionDef("nop", 0x50, LOCAL_KIND, opens_cell=True),
    InstructionDef("absorb", 0x51, LOCAL_KIND, opens_cell=True),
    InstructionDef("emit", 0x52, LOCAL_KIND, base_cost=1),
    InstructionDef("read", 0x53, LOCAL_KIND),
    InstructionDef("write", 0x54, LOCAL_KIND, base_cost=1),
    InstructionDef("del", 0x55, LOCAL_KIND, base_cost=1),
    InstructionDef("synthesize", 0x56, LOCAL_KIND, base_cost=1),
    InstructionDef("readAdj", 0x57, NONLOCAL_KIND),
    InstructionDef("writeAdj", 0x58, NONLOCAL_KIND, base_cost=1, requires_protection_break=True),
    InstructionDef("appendAdj", 0x59, NONLOCAL_KIND, base_cost=1, requires_protection_break=True),
    InstructionDef("delAdj", 0x5A, NONLOCAL_KIND, base_cost=1, requires_protection_break=True),
    InstructionDef("senseSize", 0x5B, NONLOCAL_KIND),
    InstructionDef("senseE", 0x5C, NONLOCAL_KIND),
    InstructionDef("senseM", 0x5D, NONLOCAL_KIND),
    InstructionDef("senseID", 0x5E, NONLOCAL_KIND),
    InstructionDef("giveE", 0x5F, NONLOCAL_KIND),
    InstructionDef("giveM", 0x60, NONLOCAL_KIND, base_cost=1),
    InstructionDef("takeE", 0x61, NONLOCAL_KIND, base_cost=1, requires_protection_break=True),
    InstructionDef("takeM", 0x62, NONLOCAL_KIND, base_cost=1, requires_protection_break=True),
    InstructionDef("move", 0x63, NONLOCAL_KIND, base_cost=1),
    InstructionDef("boot", 0x64, NONLOCAL_KIND),
)

OPCODE_TO_DEF = {instruction.opcode: instruction for instruction in FIXED_INSTRUCTIONS}
NAME_TO_DEF = {instruction.name.lower(): instruction for instruction in FIXED_INSTRUCTIONS}
PROTECTION_REQUIRED = {
    instruction.name
    for instruction in FIXED_INSTRUCTIONS
    if instruction.requires_protection_break
}
NONLOCAL_MNEMONICS = {
    instruction.name
    for instruction in FIXED_INSTRUCTIONS
    if instruction.kind == NONLOCAL_KIND
}
OPCODE_KIND = [IMMEDIATE_KIND] * 256
OPCODE_BASE_COST = [0] * 256
OPCODE_NAME = ["noop"] * 256
OPCODE_IS_NOOP = [True] * 256
PUSH_LITERAL = [0] * 256

for opcode in range(0x10):
    PUSH_LITERAL[opcode] = opcode if opcode < 8 else opcode - 16
    OPCODE_NAME[opcode] = "push"
    OPCODE_IS_NOOP[opcode] = False

for instruction in FIXED_INSTRUCTIONS:
    OPCODE_KIND[instruction.opcode] = instruction.kind
    OPCODE_BASE_COST[instruction.opcode] = instruction.base_cost
    OPCODE_NAME[instruction.opcode] = instruction.name
    OPCODE_IS_NOOP[instruction.opcode] = False


@dataclass(frozen=True, slots=True)
class DecodedInstruction:
    opcode: int
    name: str
    kind: str
    base_cost: int = 0
    literal: int | None = None
    opens_cell: bool = False
    is_noop: bool = False


def decode_instruction(opcode: int) -> DecodedInstruction:
    opcode = int(opcode) & 0xFF
    if 0 <= opcode <= 0x0F:
        literal = opcode if opcode < 8 else opcode - 16
        return DecodedInstruction(
            opcode=opcode,
            name="push",
            kind=IMMEDIATE_KIND,
            literal=literal,
        )
    instruction = OPCODE_TO_DEF.get(opcode)
    if instruction is None:
        return DecodedInstruction(
            opcode=opcode,
            name="noop",
            kind=IMMEDIATE_KIND,
            is_noop=True,
        )
    return DecodedInstruction(
        opcode=instruction.opcode,
        name=instruction.name,
        kind=instruction.kind,
        base_cost=instruction.base_cost,
        opens_cell=instruction.opens_cell,
    )


def encode_push(value: int) -> int:
    if not -8 <= int(value) <= 7:
        raise ValueError("push literal must be between -8 and 7.")
    return int(value) & 0x0F


def assemble_program(source: str) -> list[int]:
    bytecode: list[int] = []
    for line_no, raw_line in enumerate(source.splitlines(), start=1):
        line = raw_line.split(";", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        mnemonic = parts[0]
        lowered = mnemonic.lower()
        if lowered == "push":
            if len(parts) != 2:
                raise ValueError(f"Line {line_no}: push requires a literal.")
            bytecode.append(encode_push(int(parts[1], 0)))
            continue
        if lowered == ".byte":
            if len(parts) != 2:
                raise ValueError(f"Line {line_no}: .byte requires a numeric literal.")
            value = int(parts[1], 0)
            if not 0 <= value <= 0xFF:
                raise ValueError(f"Line {line_no}: .byte value must be between 0 and 255.")
            bytecode.append(value)
            continue
        instruction = NAME_TO_DEF.get(lowered)
        if instruction is None:
            raise ValueError(f"Line {line_no}: unknown instruction {mnemonic!r}.")
        if len(parts) != 1:
            raise ValueError(f"Line {line_no}: {instruction.name} does not take operands.")
        bytecode.append(instruction.opcode)
    if not bytecode:
        raise ValueError("Program cannot be empty.")
    return bytecode


def disassemble_program(bytecode: list[int]) -> list[str]:
    lines: list[str] = []
    for raw in bytecode:
        instruction = decode_instruction(raw)
        if instruction.name == "push":
            lines.append(f"push {instruction.literal}")
        elif instruction.is_noop:
            lines.append(f".byte 0x{instruction.opcode:02x} ; noop")
        else:
            lines.append(instruction.name)
    return lines


def normalize_assembly(source: str) -> str:
    return "\n".join(disassemble_program(assemble_program(source))) + "\n"
