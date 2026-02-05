//@ compile-flags: -Z deduplicate-diagnostics=yes

#![feature(fn_delegation)]
#![allow(incomplete_features)]

mod test_1 {
    trait Trait<'a> {}
    impl<'a> Trait<'a> for usize {}

    fn foo<'a: 'static, 'b: 'a>(_s: &'a str, _t: &'a dyn Trait<'b>) {}

    reuse foo as bar;

    pub fn check() {
        bar("", &1);

        let x = 1;
        bar("", &x);
        //~^ ERROR: `x` does not live long enough
    }
}

mod test_2 {
    fn foo<'a, 'b>(_s: &'a str, _t: &'b usize) {}

    reuse foo as bar;

    reuse foo::<'static> as error;
    //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

    reuse foo::<'static, 'static> as error2;
    //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

    pub fn check<'a, 'b>(s: &'a str, x: &'b usize) {
        bar::<'static>(s, x);
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

        bar::<'a, 'b>(s, x);
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }
}

mod test_3 {
    fn foo<'a, 'b>() {}

    reuse foo as bar;

    reuse foo::<'static> as error;
    //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

    reuse foo::<'static, 'static> as error2;
    //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

    pub fn check<'a, 'b>(s: &'a str, x: &'b usize) {
        bar::<'a>();
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

        bar::<'a, 'b>();
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }
}

mod test_4 {
    trait Trait<'x, 'y> {
        fn foo<'a, 'b>(&self, _s: &'a str, _t: &'b usize) {}
    }

    impl<'x, 'y> Trait<'x, 'y> for usize {}

    reuse Trait::foo as bar;
    reuse Trait::<'static, 'static>::foo as bar2;

    reuse Trait::foo::<'static, 'static> as bar3;
    //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    //~| WARN: this was previously accepted by the compiler but is being phased out
    //~| WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

    pub fn check<'a, 'b>(s: &'a str, x: &'b usize) {
        bar(&123, s, x);
        bar2(&123, "", &1);

        bar::<'static, 'static, usize>(&123, s, x);
        //~^ WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
        //~| WARN: this was previously accepted by the compiler but is being phased out
    }
}

mod test_5 {
    trait Trait<'x, 'y> {
        fn foo<'a, 'b>(&self) {}
    }

    struct F;
    impl<'x, 'y> Trait<'x, 'y> for F {}

    struct S<'a, 'b, 'c, A, B, const C: bool>(F, &'a A, &'b B, &'c B);

    impl<'a, 'b, 'c, A, B, const C: bool> Trait<'b, 'c> for S<'a, 'b, 'c, A, B, C> {
        reuse Trait::foo::<'a, 'b> { &self.0 }
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
        //~| ERROR: lifetime parameters or bounds on method `foo` do not match the trait declaration
        //~| WARN: this was previously accepted by the compiler but is being phased out
        //~| WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }

    pub fn check() {
        let s = S(F, &123, &123, &123);
        <S::<'static, 'static, 'static, i32, i32, true> as Trait>::foo(&s);
    }
}

mod test_6 {
    trait Trait<'x, 'y> {
        fn foo<'a, 'b>(&self) {}
    }

    struct F;
    impl<'x, 'y> Trait<'x, 'y> for F {}

    struct S<'a, 'b, 'c, A, B>(F, &'a A, &'b B, &'c B);
    impl<'a, 'b, 'c, A, B> S<'a, 'b, 'c, A, B> {
        reuse Trait::foo { &self.0 }
        reuse Trait::<'c, 'a>::foo as bar { &self.0 }
        reuse Trait::<'c, 'a>::foo::<'a, 'b> as error { &self.0 }
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }

    pub fn check() {
        let s = S(F, &123, &123, &123);

        S::<'static, 'static, 'static, i32, i32>::foo(&s);
        S::<'static, 'static, 'static, i32, i32>::foo::<'static>(&s);
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present

        s.foo();
    }
}

mod test_7 {
    trait Trait<'x, 'y> {
        fn foo<'a, 'b>(&self) {}
    }

    impl<'x, 'y> Trait<'x, 'y> for u8 {}

    trait Trait2<'a, 'b> : Trait<'a, 'b> {
        fn get() -> &'static u8 { &0 }
        fn get_self(&self) -> &'static u8 { &0 }

        reuse Trait::foo { Self::get() }
        reuse Trait::<'a, 'b>::foo as bar { self.get_self() }

        reuse Trait::<'a, 'b>::foo::<'static, 'a> as error;
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
        reuse Trait::foo::<'b> as error2;
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
        //~| WARN: this was previously accepted by the compiler but is being phased out
        //~| WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }

    impl<'x, 'y> Trait<'x, 'y> for u32 {}
    impl<'x, 'y> Trait2<'x, 'y> for u32 {}

    pub fn check() {
        <u32 as Trait2<'static, 'static>>::foo::<'static>(&123);
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
        <u32 as Trait2<'static, 'static>>::bar::<'static>(&123);
        //~^ ERROR: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
    }
}

fn main() {
    test_1::check();
    test_2::check("", &1);
    test_3::check("", &1);
    test_4::check("", &1);
    test_5::check();
    test_6::check();
    test_7::check();
}
