macro_rules! impl_error {
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name(pub String);

        impl std::error::Error for $name {}

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

macro_rules! impl_from_error {
    ($source:ty, $target:ty) => {
        impl From<$source> for $target {
            fn from(err: $source) -> Self {
                Self(err.to_string())
            }
        }
    };
}

pub(crate) use impl_error;
pub(crate) use impl_from_error;
