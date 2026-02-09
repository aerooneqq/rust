#![feature(fn_delegation)]
#![allow(incomplete_features)]

mod test_1 {
    trait Trait<'b, T>: Sized {
        fn foo<U, const M: bool>(&self) {}
    }

    impl<'a, T> Trait<'a, T> for u8 {}

    reuse Trait::foo::<String, false> as bar1;
    reuse Trait::foo as bar2;
    reuse Trait::foo::<String, false> as bar3;
    reuse Trait::foo as bar4;

    reuse <u8 as Trait>::foo::<String, false> as bar5;
    reuse <u8 as Trait>::foo as bar6;
    reuse <u8 as Trait>::foo::<String, false> as bar7;
    reuse <u8 as Trait>::foo as bar8;

    reuse Trait::<'static, usize>::foo::<String, false> as bar9;
    reuse Trait::<'static, usize>::foo as bar10;
    reuse Trait::<'static, usize>::foo::<String, false> as bar11;
    reuse Trait::<'static, usize>::foo as bar12;

    reuse <u8 as Trait::<'static, usize>>::foo::<String, false> as bar13;
    reuse <u8 as Trait::<'static, usize>>::foo as bar14;
    reuse <u8 as Trait::<'static, usize>>::foo::<String, false> as bar15;
    reuse <u8 as Trait::<'static, usize>>::foo as bar16;

    trait Trait2<'a, 'b, 'c, X, Y, Z>: Sized {
        fn get() -> &'static u8 { &0 }
        fn get_self(&self) -> &'static u8 { &0 }

        reuse Trait::foo::<String, false> as bar1 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar2 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo::<String, false> as bar3 { self.get_self() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar4 { self.get_self() }
        //~^ ERROR: type annotations needed

        reuse <u8 as Trait>::foo::<String, false> as bar5 { Self::get() }
        reuse <u8 as Trait>::foo as bar6 { Self::get() }
        reuse <u8 as Trait>::foo::<String, false> as bar7 { self.get_self() }
        reuse <u8 as Trait>::foo as bar8 { self.get_self() }

        reuse Trait::<'static, usize>::foo::<String, false> as bar9 { Self::get() }
        reuse Trait::<'static, usize>::foo as bar10 { Self::get() }
        reuse Trait::<'static, usize>::foo::<String, false> as bar11 { self.get_self() }
        reuse Trait::<'static, usize>::foo as bar12 { self.get_self() }
    }

    struct X;

    impl X {
        fn get() -> &'static u8 { &0 }
        fn get_self(&self) -> &'static u8 { &0 }

        reuse Trait::foo::<String, false> as bar1 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar2 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo::<String, false> as bar3 { self.get_self() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar4 { self.get_self() }
        //~^ ERROR: type annotations needed

        reuse <u8 as Trait>::foo::<String, false> as bar5 { Self::get() }
        reuse <u8 as Trait>::foo as bar6 { Self::get() }
        reuse <u8 as Trait>::foo::<String, false> as bar7 { self.get_self() }
        reuse <u8 as Trait>::foo as bar8 { self.get_self() }

        reuse Trait::<'static, usize>::foo::<String, false> as bar9 { Self::get() }
        reuse Trait::<'static, usize>::foo as bar10 { Self::get() }
        reuse Trait::<'static, usize>::foo::<String, false> as bar11 { self.get_self() }
        reuse Trait::<'static, usize>::foo as bar12 { self.get_self() }
    }
}

mod test_2 {
    trait Trait<'b, T>: Sized {
        fn foo<U, const M: bool>(&self) {}
    }

    impl Trait<'static, usize> for u8 {}

    trait Trait2<'a, 'b, 'c, X, Y, Z>: Sized {
        fn get() -> &'static u8 { &0 }
        fn get_self(&self) -> &'static u8 { &0 }

        reuse Trait::foo::<String, false> as bar1 { Self::get() }
        reuse Trait::foo as bar2 { Self::get() }
        reuse Trait::foo::<String, false> as bar3 { self.get_self() }
        reuse Trait::foo as bar4 { self.get_self() }
    }
}

mod test_3 {
    trait Trait<'b, T>: Sized {
        fn foo<U, const M: bool>(&self) {}
    }

    impl Trait<'static, usize> for u8 {}
    impl Trait<'static, String> for u8 {}

    trait Trait2<'a, 'b, 'c, X, Y, Z>: Sized {
        fn get() -> &'static u8 { &0 }
        fn get_self(&self) -> &'static u8 { &0 }

        reuse Trait::foo::<String, false> as bar1 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar2 { Self::get() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo::<String, false> as bar3 { self.get_self() }
        //~^ ERROR: type annotations needed
        reuse Trait::foo as bar4 { self.get_self() }
        //~^ ERROR: type annotations needed
    }
}

fn main() {
}