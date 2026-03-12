# Proteus

Proteus is an artificial life experiment and 2D cellular automaton where programs and computation are the fundamental "stuff" from which life emerges. It takes inspiration from numerous projects, including Tierra and The Life Engine. Design goals include:
- A believable, elegant system of physics in which inert and living matter are not fundamentally separate from one other
- Program instructions and computational resources as conserved quantities
- Scalable implementation with the potential for distributed computation

## Design snapshot

- 2D grid, 4-neighbor interactions, discrete time steps
- Computation is implemented by a Forth-like stack machine with specialized registers for interacting with the wider world
- Each cell contains a program state (instructions, registers, stack) and energy
- Single byte instruction opcodes are the fundamental units of mass
- Energy is required to execute and modify instructions, and to communicate with other cells
- Update rules are fully discrete and determinstic (aside from mutations and random arrivals of energy and mass from outside the system)
