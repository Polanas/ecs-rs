use macro_rules_attribute::apply;
#[apply(impl_component!)]
#[derive(Copy, Debug, Default)]
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
#[apply(impl_component!)]
#[derive(Copy, Debug, Default)]
pub struct Velocity {
    pub x: i32,
    pub y: i32,
}

impl Velocity {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[apply(impl_component!)]
pub struct Name {
    pub value: String,
}
#[apply(impl_component!)]
pub struct Owes {
    pub amount: i32,
}
#[apply(impl_component!)]
pub struct IsCool {}
#[apply(impl_component!)]
pub struct Likes {}
#[apply(impl_component!)]
pub struct Begin {}
#[apply(impl_component!)]
pub struct End {}
#[apply(impl_component!)]
pub struct Apples {}
#[apply(impl_component!)]
pub struct Oranges {}
