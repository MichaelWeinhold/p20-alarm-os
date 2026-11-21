#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    VolumeUp,
    VolumeDown,
    Power,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputEvent {
    Touch { x: usize, y: usize },
    Move { x: usize, y: usize },
    Lift,
    Press(Key),
}
