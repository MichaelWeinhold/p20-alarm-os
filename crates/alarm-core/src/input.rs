#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    VolumeUp,
    VolumeDown,
    Power,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputEvent {
    Touch { x: usize, y: usize },
    Press(Key),
}
