use crate::types::{Direction, Message};

#[derive(Clone, Debug)]
pub struct DirectedRadiation {
    direction: Direction,
    message: Message,
}
