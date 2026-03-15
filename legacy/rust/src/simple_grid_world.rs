// 1. 2d grid of cells backed by 1d vec of usize (energy). initialized to 0.
// 2. each cell has random propensity to choose action move/grow/stay. if move, then it chooses a
//    direction with equal probability.
// 3. if action is direction, then decrease energy by 1 (cancel if can't afford), but increase if
//    grow.
// 4. cell is vulnerable if it grew or failed to pay cost. all actions targeting non-vulnerable
//    cells fail
// 5. if two cells target each other, bigger one wins, or fail on tie
// 6. if a cell is targeted by multiple adjacent cells, then only the cell with highest value wins.
//    if tie, then all fail.
// 7. when a cell successfully moves, half its energy (rounded down) is moved to the new cell.
