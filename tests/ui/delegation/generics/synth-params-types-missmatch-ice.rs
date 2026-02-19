#![feature(fn_delegation)]
#![allow(incomplete_features)]

fn foo<T, const N: bool>(x: &impl Trait<T, T, T>) {}

trait Trait<A, B, C> {
    fn get_self(&self) -> Self;

    reuse foo as bar { self.get_self() }
    //~^ ERROR: mismatched types
    reuse foo::<A, true> as bar1 { self.get_self() }
    //~^ ERROR: mismatched types
}

fn main() {}
