use crate::types::{Direction, Message};

const INITIAL_STACK_CAPACITY: usize = 8;
const MAX_STACK_CAPACITY: usize = i16::MAX as usize;

#[derive(Clone, Debug)]
pub struct Stack {
    values: Vec<i16>,
}

#[derive(Debug, PartialEq)]
pub enum StackError {
    Overflow,
    Underflow,
}

impl Stack {
    pub fn new() -> Self {
        Stack {
            values: Vec::with_capacity(INITIAL_STACK_CAPACITY),
        }
    }

    pub fn push(&mut self, value: i16) -> Result<(), StackError> {
        if self.values.len() >= MAX_STACK_CAPACITY {
            return Err(StackError::Overflow);
        }
        self.values.push(value);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<i16, StackError> {
        self.values.pop().ok_or(StackError::Underflow)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn dup(&mut self) -> Result<(), StackError> {
        if let Some(&value) = self.values.last() {
            self.push(value)
        } else {
            Err(StackError::Underflow)
        }
    }

    pub fn swap(&mut self) -> Result<(), StackError> {
        if self.values.len() < 2 {
            return Err(StackError::Underflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

// CPU state for a program
#[derive(Clone, Debug)]
pub struct CPU {
    pub stack: Stack,
    // Program ID
    pub id: i8,
    // READ-ONLY REGISTERS
    // Plasmid pointer
    pub pp: i8,
    // Instruction pointer
    pub ip: i16,
    // Error/message received flag
    pub flag: bool,
    // Loop counter
    pub lc: i16,
    // Message
    pub msg: Message,
    // Message received from direction
    pub msg_dir: Direction,
    // TARGETING REGISTERS
    // Direction if targeting adjacent cell
    pub dir: Direction,
    // Target adjacent cell (true) or self (false)
    pub adj: bool,
    // Plasmid offset
    pub po: i8,
    // Instruction offset
    pub io: i16,
    // Label
    pub lab: i8,
}

impl Default for CPU {
    fn default() -> Self {
        Self {
            stack: Stack::new(),
            id: 0,
            pp: 0,
            ip: 0,
            flag: false,
            lc: 0,
            msg: 0,
            msg_dir: Default::default(),
            dir: Default::default(),
            adj: false,
            po: 0,
            io: 0,
            lab: -1,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_operations() {
        let mut stack = Stack::new();

        // Test push and pop
        assert_eq!(stack.pop(), Err(StackError::Underflow));
        assert_eq!(stack.push(42), Ok(()));
        assert_eq!(stack.pop(), Ok(42));
        assert_eq!(stack.pop(), Err(StackError::Underflow));

        // Test dup
        assert_eq!(stack.dup(), Err(StackError::Underflow)); // Empty stack
        assert_eq!(stack.push(123), Ok(()));
        assert_eq!(stack.dup(), Ok(()));
        assert_eq!(stack.pop(), Ok(123));
        assert_eq!(stack.pop(), Ok(123));

        // Test swap
        assert_eq!(stack.push(1), Ok(()));
        assert_eq!(stack.swap(), Err(StackError::Underflow)); // Need 2 elements
        assert_eq!(stack.push(2), Ok(()));
        assert_eq!(stack.swap(), Ok(()));
        assert_eq!(stack.pop(), Ok(1));
        assert_eq!(stack.pop(), Ok(2));
    }

    #[test]
    fn test_stack_overflow() {
        let mut stack = Stack::new();
        for _ in 0..MAX_STACK_CAPACITY {
            assert_eq!(stack.push(0), Ok(()));
        }
        assert_eq!(stack.push(0), Err(StackError::Overflow));
        assert_eq!(stack.pop(), Ok(0));
    }
}
