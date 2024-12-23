use crate::types::{Direction, Message};

const INITIAL_STACK_CAPACITY: usize = 8;
const MAX_STACK_CAPACITY: usize = i16::MAX as usize;

pub type CPUResult<T> = Result<T, CPUError>;

#[derive(Clone, Debug)]
pub struct Stack {
    values: Vec<i16>,
}

#[derive(Debug, PartialEq)]
pub enum CPUError {
    StackOverflow,
    StackUnderflow,
    // Add other specific variants as needed
}

impl Stack {
    pub fn new() -> Self {
        Stack {
            values: Vec::with_capacity(INITIAL_STACK_CAPACITY),
        }
    }

    pub fn push(&mut self, value: i16) -> CPUResult<()> {
        if self.values.len() >= MAX_STACK_CAPACITY {
            return Err(CPUError::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    pub fn pop(&mut self) -> CPUResult<i16> {
        self.values.pop().ok_or(CPUError::StackUnderflow)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn dup(&mut self) -> CPUResult<()> {
        let value = self.values.last().ok_or(CPUError::StackUnderflow)?;
        self.push(*value)
    }

    pub fn swap(&mut self) -> CPUResult<()> {
        if self.values.len() < 2 {
            return Err(CPUError::StackUnderflow);
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

impl CPU {
    pub fn push(&mut self, value: i16) -> CPUResult<()> {
        self.stack.push(value)
    }

    pub fn pop(&mut self) -> CPUResult<i16> {
        self.stack.pop()
    }

    pub fn add(&mut self) -> CPUResult<()> {
        let b = self.pop()?;
        let a = self.pop()?;
        // TODO: handle arithmetic overflow?
        self.push(a + b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_operations() {
        let mut stack = Stack::new();

        // Test push and pop
        assert_eq!(stack.pop(), Err(CPUError::StackUnderflow));
        assert_eq!(stack.push(42), Ok(()));
        assert_eq!(stack.pop(), Ok(42));
        assert_eq!(stack.pop(), Err(CPUError::StackUnderflow));

        // Test dup
        assert_eq!(stack.dup(), Err(CPUError::StackUnderflow)); // Empty stack
        assert_eq!(stack.push(123), Ok(()));
        assert_eq!(stack.dup(), Ok(()));
        assert_eq!(stack.pop(), Ok(123));
        assert_eq!(stack.pop(), Ok(123));

        // Test swap
        assert_eq!(stack.push(1), Ok(()));
        assert_eq!(stack.swap(), Err(CPUError::StackUnderflow)); // Need 2 elements
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
        assert_eq!(stack.push(0), Err(CPUError::StackOverflow));
        assert_eq!(stack.pop(), Ok(0));
    }
}
