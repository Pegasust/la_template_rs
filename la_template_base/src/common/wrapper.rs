#[macro_export]
macro_rules! wrapper {
    (#[$($attr:meta),*] $new_v:vis $new_type:ident wraps $old_v:vis $wrapped_type:ty) => {
        $(#[$attr])*
        #[repr(transparent)]
        $new_v struct $new_type($old_v $wrapped_type);
        impl $new_type {
            $new_v fn get_ref(&self) -> &$wrapped_type {&self.0}
            $new_v fn get_mut(&mut self) -> &mut $wrapped_type {&mut self.0}
            $new_v fn move_inner(self) -> $wrapped_type {self.0}
        }
        impl From<$wrapped_type> for $new_type {
            fn from(old: $wrapped_type) -> Self {
                Self(old)
            }
        }
        impl From<$new_type> for $wrapped_type {
            fn from(newt: $new_type) -> Self {
                newt.0
            }
        }
    }
}

#[macro_export]
macro_rules! wrap_fn {
    // move
    ($v:vis fn $func:ident($self:ident $(,$arg:ident: $argt:ty),*) -> $ret:ty) => {
        fn $func($self $(,$arg: $argt),*) -> $ret {
            $self.0.$func($($arg),*)
        }
    };
    // ref
    ($v:vis fn $func:ident(&$self:ident $(,$arg:ident: $argt:ty),*) -> $ret:ty) => {
        fn $func(&$self $(,$arg: $argt),*) -> $ret {
            $self.0.$func($($arg),*)
        }
    };
    // mut ref
    ($v:vis fn $func:ident(&mut $self:ident $(,$arg:ident: $argt:ty),*) -> $ret:ty) => {
        fn $func(&mut $self $(,$arg: $argt),*) -> $ret {
            $self.0.$func($($arg),*)
        }
    };
}