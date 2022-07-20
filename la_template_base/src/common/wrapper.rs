#[macro_export]
macro_rules! wrapper {
    ($new_v:vis $new_type:ident wraps $old_v:vis $wrapped_type:ident) => {
        #[repr(transparent)]
        $new_v struct $new_type($old_v $wrapped_type);
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