//@ run-pass

#![feature(fn_delegation)]
#![allow(incomplete_features)]
#![allow(warnings)]

// mod test_1 {
//     fn foo<'a: 'a, T, const N: bool>(x: usize) -> usize {
//         x + N as usize
//     }

//     reuse foo as bar { self + 1 }

//     reuse bar::<'static, i32, true> as oof { self + 1}

//     reuse oof as final_f { self + 1 }


//     pub fn check() {
//         assert_eq!(bar::<'static, i32, false>(1), 2);
//         assert_eq!(oof(1), 4);
//         assert_eq!(final_f(2), 6);
//     }
// }

// mod test_2 {
//     fn foo<'a: 'a, T, const N: bool>() {}

//     trait Trait<'a, A, B, C> {
//         reuse foo as bar;
//         reuse foo::<'static, A, true> as bar1;
//     }

//     impl<'a, A, B, C> Trait<'a, A, B, C> for usize {} 

//     reuse <usize as Trait>::bar::<'static, i32, true> as reuse;
//     reuse <usize as Trait>::bar as reuse1;
//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar::<'static, i32, true> as reuse2;
//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar as reuse3;

//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar1 as reuse4;
//     reuse <usize as Trait>::bar1 as reuse5;

//     pub fn check() {
//         reuse::<'static, i32, i32, i32>();
//         reuse1::<'static, 'static, i32, i32, i32, String, false>();
//         reuse2();
//         reuse3::<'static, &str, true>();
//         reuse4();
//         reuse5::<'static, i32, i32, i32>();
//     }
// }

// mod test_3 {
//     fn foo<'a: 'a, T, const N: bool>(_x: impl Trait<'a, T, T, T>) {}

//     trait Trait<'a, A, B, C> {
//         fn get_self(&self) -> Self;

//         reuse foo as bar { self.get_self() }
//         reuse foo::<'static, A, true> as bar1 { self.get_self() }
//     }

//     impl<'a, A, B, C> Trait<'a, A, B, C> for usize {
//         fn get_self(&self) -> usize { 123 }
//     }

//     reuse <usize as Trait>::bar::<'static, i32, true> as reuse;
//     reuse <usize as Trait>::bar as reuse1;
//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar::<'static, i32, true> as reuse2;
//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar as reuse3;

//     reuse <usize as Trait::<'static, i32, i32, i32>>::bar1 as reuse4;
//     reuse <usize as Trait>::bar1 as reuse5;

//     pub fn check() {
//         reuse::<'static, i32, i32, i32>(123);
//         reuse1::<'static, 'static, i32, i32, i32, String, false>(123);
//         reuse2(123);
//         reuse3::<'static, &str, true>(123);
//         reuse4(123);
//         reuse5::<'static, i32, i32, i32>(123);
//     }
// }

// mod test_4 {
//     trait Trait<'a, A, const N: usize> {
//         fn foo<'b: 'b, X, const B: bool>(&self) -> usize { 123 }
//     }

//     impl Trait<'static, i32, 1> for () {}

//     impl<'a, A, const N: usize> Trait<'a, A, N> for ((), ()) {}

//     reuse Trait::foo as foo;
//     reuse Trait::<'static, i32, 1>::foo as foo1;
//     reuse Trait::<'static, i32, 1>::foo::<'static, String, false> as foo2;
//     reuse Trait::foo::<'static, String, false> as foo3;

//     reuse <((), ()) as Trait>::foo as bar;
//     reuse <((), ()) as Trait::<'static, i32, 1>>::foo as bar1;
//     reuse <((), ()) as Trait::<'static, i32, 1>>::foo::<'static, String, false> as bar2;
//     reuse <((), ()) as Trait>::foo::<'static, String, false> as bar3;

//     struct X<A, B>((), A, B);
//     impl<A, B> X<A, B> {
//         reuse foo;
//         reuse foo1;
//         reuse foo2;
//         reuse foo3;
//         reuse foo::<'static, 'static, (), i32, 1, String, false> as foo4;
//         reuse foo1::<'static, (), String, false> as foo5;
//         reuse foo2::<()> as foo6;
//         reuse foo3::<'static, (), i32, 1> as foo7;

//         reuse bar;
//         reuse bar1;
//         reuse bar2;
//         reuse bar3;
//         reuse bar::<'static, 'static, i32, 1, String, false> as bar4;
//         reuse bar1::<'static, String, false> as bar5;
//         reuse bar2 as bar6;
//         reuse bar3::<'static, i32, 1> as bar7;
//     }

//     pub fn check() {
//         X::<(), ()>::foo::<'static, 'static, (), i32, 1, String, false>(&());
//         X::<(), ()>::foo1::<'static, (), String, false>(&());
//         X::<(), ()>::foo2::<()>(&());
//         X::<(), ()>::foo3::<'static, (), i32, 1>(&());
//         X::<(), ()>::foo4(&());
//         X::<(), ()>::foo5(&());
//         X::<(), ()>::foo6(&());
//         X::<(), ()>::foo7(&());

//         X::<(), ()>::bar::<'static, 'static, i32, 1, String, false>(&((), ()));
//         X::<(), ()>::bar1::<'static, String, false>(&((), ()));
//         X::<(), ()>::bar2(&((), ()));
//         X::<(), ()>::bar3::<'static, i32, 1, >(&((), ()));
//         X::<(), ()>::bar4(&((), ()));
//         X::<(), ()>::bar5(&((), ()));
//         X::<(), ()>::bar6(&((), ()));
//         X::<(), ()>::bar7(&((), ()));
//     }
// }

mod test_5 {
    trait Trait1<'a, A, const N: usize> {
        fn foo<'b: 'b, T, const B: bool>(&self, x: usize) -> usize { x }
    }

    impl Trait1<'static, String, 123> for () {}

    trait Trait2 {
        fn get_trait1(&self) -> () { () }

        reuse Trait1::foo { self.get_trait1() }
    }

    impl Trait2 for () {}

    trait Trait3 {
        fn get_trait2(&self) -> () { () }

        reuse Trait2::foo::<'static, String, false> { self.get_trait2() }
    }

    trait Trait4 {

    }

    pub fn check() {

    }
}

fn main() {
    // test_1::check();
    // test_2::check();
    // test_3::check();
    // test_4::check();
    // test_5::check();
}
