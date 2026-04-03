#![feature(fn_delegation)]
#![allow(incomplete_features)]

mod test_1 {
    struct X;

    impl X {
        fn foo() -> usize {
            123
        }
    }

    #[derive(PartialEq, Eq, Debug)]
    enum Animal {
        Cat,
        Dog,
    }

    impl Animal {
        pub fn foo(self) -> Animal {
            self
        }
    }

    reuse X::foo;
    reuse Animal::foo as bar { Animal::Dog }

    pub fn check() {
        assert_eq!(foo(), 123);
        assert_eq!(bar(Animal::Cat), Animal::Dog);
    }
}

// Const evaluation forces query cycle
mod test_2 {
    struct X<'a, T, const N: usize = 123>(&'a T, &'a [T; N]);

    impl<'a, T, const N: usize> X<'a, T, const N: usize> {
        //~^ ERROR: unexpected `const` parameter declaration
        //~| ERROR: the const parameter `N` is not constrained by the impl trait, self type, or predicates
        fn foo() -> usize {
            123
        }
    }

    reuse X::foo;
    //~^ ERROR: type annotations needed
}

// No error emitted but delayed bug is spawned
mod test_3 {
    trait Trait {
        fn foo(&self) {}
    }

    trait Trait1 {
        fn foo(&self) {}
    }

    struct X;

    impl Trait for X {
        fn foo(&self) {}
    }

    impl Trait1 for X {
        fn foo(&self) {}
    }

    reuse X::foo;
    //~^ ERROR: failed to resolve delegation
}

// No query cycles when there are generics errors, methods are resolved, no errors
// about not full-filled obligations
mod test_4 {
    struct X<T>; //~ ERROR: type parameter `T` is never used

    impl X { //~ ERROR: missing generics for struct `test_4::X`
        fn foo() -> usize {
            123
        }
    }

    #[derive(PartialEq, Eq, Debug)]
    enum Animal<T> {
        Cat,
        Dog(T),
    }

    impl Animal { //~ ERROR: missing generics for enum `test_4::Animal`
        pub fn foo(self) -> Animal { //~ ERROR: missing generics for enum `test_4::Animal`
            self
        }
    }

    reuse X::foo;
    reuse Animal::foo as bar { Animal::Dog }

    pub fn check() {
        assert_eq!(foo(), 123);
        assert_eq!(bar(Animal::Cat), Animal::Dog);
        //~^  ERROR: `fn(_) -> test_4::Animal<_> {test_4::Animal::<_>::Dog}` doesn't implement `Debug`
    }
}

mod test_5 {
    struct X;

    impl X {
        reuse Y::foo;
        //~^ ERROR: failed to resolve delegation
    }

    struct Y;

    impl Y {
        reuse X::foo;
        //~^ ERROR: failed to resolve delegation
    }

    fn check() {
        X::foo();
        Y::foo();
    }
}

mod test_6 {
    struct X;

    impl X {
        fn foo() {}
    }

    struct Y;
    impl Y {
        reuse X::foo as bar;
    }

    struct Z;
    impl Z {
        reuse Y::bar;
    }

    reuse Z::bar;

    fn check() {
        Y::bar();
        Z::bar();
        bar();
    }
}

fn main() {
    test_1::check();
    test_4::check();
}
