# cython: language_level=3, boundscheck=False, wraparound=False, initializedcheck=False, nonecheck=False

from proteus.spec import OPCODE_IS_NOOP


def normalize_program_for_hash(list instructions, int canonical_noop_opcode):
    cdef Py_ssize_t size = len(instructions)
    cdef bytearray normalized = bytearray(size)
    cdef Py_ssize_t index
    cdef int opcode

    for index in range(size):
        opcode = (<int>instructions[index]) & 0xFF
        normalized[index] = canonical_noop_opcode if bool(OPCODE_IS_NOOP[opcode]) else opcode
    return bytes(normalized)


def scan_forward_opcode(list instructions, int start_ip, int target_opcode):
    cdef Py_ssize_t size = len(instructions)
    cdef Py_ssize_t index
    cdef Py_ssize_t step

    if size == 0:
        return -1

    index = (start_ip + 1) % size
    for step in range(size):
        if ((<int>instructions[index]) & 0xFF) == target_opcode:
            return index
        index += 1
        if index == size:
            index = 0
    return -1


def scan_backward_opcode(list instructions, int start_ip, int target_opcode):
    cdef Py_ssize_t size = len(instructions)
    cdef Py_ssize_t index
    cdef Py_ssize_t step

    if size == 0:
        return -1

    index = (start_ip - 1) % size
    for step in range(size):
        if ((<int>instructions[index]) & 0xFF) == target_opcode:
            return index
        if index == 0:
            index = size - 1
        else:
            index -= 1
    return -1
