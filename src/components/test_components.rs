use macro_rules_attribute::apply;
#[apply(Component!)]
#[derive(Copy, Default)]
pub struct Position {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
#[apply(Component!)]
#[derive(Copy, Default)]
pub struct Velocity {
    pub x: i32,
    pub y: i32,
}

impl Velocity {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[apply(Component!)]
pub struct Name {
    pub value: String,
}
#[apply(Component!)]
pub struct Owes {
    pub amount: i32,
}
#[apply(Component!)]
pub struct IsCool {}
#[apply(Component!)]
pub struct Likes {}
#[apply(Component!)]
pub struct Begin {}
#[apply(Component!)]
pub struct End {}
#[apply(Component!)]
pub struct Apples {}
#[apply(Component!)]
pub struct Oranges {}
