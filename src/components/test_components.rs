use bevy_reflect::Reflect;
impl_component! {
    #[derive(Copy, Debug, Default)]
    pub struct Position {
        pub x: i32,
        pub y: i32,
    }
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
impl_component! {
    #[derive(Copy, Debug, Default)]
    pub struct Velocity {
        pub x: i32,
        pub y: i32,
    }
}

impl Velocity {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl_component! {
    pub struct Name {
        pub value: String,
    }
}
impl_component! {
    pub struct Owes {
        pub amount: i32,
    }
}

impl_component! {
    pub struct IsCool {}
}
impl_component! {
    pub struct Likes {}
}
impl_component! {
    pub struct Begin {}
}
impl_component! {
    pub struct End {}
}
impl_component! {
    pub struct Apples {}
}
impl_component! {
    pub struct Oranges {}
}
