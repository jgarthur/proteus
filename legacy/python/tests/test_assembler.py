from proteus.spec import assemble_program, normalize_assembly


def test_assemble_program_supports_push_and_raw_bytes():
    program = assemble_program("push -1\nrand\n.byte 0xaa\n")
    assert program == [0x0F, 0x14, 0xAA]


def test_normalize_assembly_marks_unknown_bytes_as_noops():
    normalized = normalize_assembly("push 0\n.byte 0xaa\n")
    assert normalized == "push 0\n.byte 0xaa ; noop\n"

