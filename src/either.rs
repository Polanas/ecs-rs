use std::fmt::Debug;

pub enum Either<A, B> {
    First(A),
    Second(B),
}

impl<A: Debug, B: Debug> Debug for Either<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::First(arg0) => f.debug_tuple("First").field(arg0).finish(),
            Self::Second(arg0) => f.debug_tuple("Second").field(arg0).finish(),
        }
    }
}

impl<A: Clone, B: Clone> Clone for Either<A, B> {
    fn clone(&self) -> Self {
        match self {
            Self::First(arg0) => Self::First(arg0.clone()),
            Self::Second(arg0) => Self::Second(arg0.clone()),
        }
    }
}

impl<A: Copy, B: Copy> Copy for Either<A, B> {}
