//@ compile-flags: -Z deduplicate-diagnostics=yes

#![allow(warnings)]
#![feature(fn_delegation)]
#![allow(incomplete_features)]

pub mod to_reuse {
    pub fn bar<'a: 'a, 'b: 'b, A, B>(x: &super::X) {}
    pub fn bar1(x: &super::X) {}
    pub fn bar2<A, B, C, D, E, F, const M: usize, const Y: bool>(x: &super::X) {}

    reuse bar::<i32> as foo;
    //~^ ERROR: function takes 2 generic arguments but 1 generic argument was supplied

    reuse bar::<'static, 'static> as foo1;
    //~^ ERROR: the placeholder `_` is not allowed within types on item signatures for functions

    reuse bar::<i32, i32, i32, i32, i32, i32, i32, i32, i32> as foo2;
    //~^ ERROR: function takes 2 generic arguments but 9 generic arguments were supplied

    reuse bar::<'static, 123, 'static, i32, i32, i32, i32, i32, i32, i32, i32, i32> as foo3;
    //~^ ERROR: function takes 2 generic arguments but 10 generic arguments were supplied
}

pub trait Trait<'a, 'b, 'c, A, B, const N: usize>: Sized {
    fn bar<'x: 'x, 'y: 'y, AA, BB, const NN: usize>(&self) {}
    fn bar1<'x: 'x, 'y: 'y, AA, BB, const NN: usize>(&self) {}
    fn bar2(&self) {}
    fn bar3(&self) {}
    fn bar4<X, Y, Z>(&self) {}
}

struct X;

impl<'a, 'b, 'c, A, B, const N: usize> Trait<'a, 'b, 'c, A, B, N> for X {
    reuse to_reuse::bar;
    //~^ ERROR: function takes at most 2 generic arguments but 3 generic arguments were supplied

    reuse to_reuse::bar1;
    //~^ ERROR: function takes 0 generic arguments but 3 generic arguments were supplied

    reuse to_reuse::bar2;
    //~^ ERROR: type annotations needed
    //~| ERROR: type annotations needed

    reuse to_reuse::bar2::<i32, i32, i32, i32, i32, i32, 123, true> as bar3;

    reuse to_reuse::bar2::<i32, i32, i32, i32, i32, i32, 123, true> as bar4;
    //~^ ERROR: method `bar4` has 0 type parameters but its trait declaration has 3 type parameters
    //~| ERROR: generic arg X is not found in delegation
    //~| ERROR: generic arg Y is not found in delegation
    //~| ERROR: generic arg Z is not found in delegation
}

struct Y;

impl<'a, 'b, 'c, A, B, const N: usize> Trait<'a, 'b, 'c, A, B, N> for Y {
    reuse Trait::<'a, 'b, 'c, A, B, N>::bar;

    reuse Trait::<'a, 'b, 'c, A, B, N>::bar1;

    reuse Trait::<'a, 'b, 'c, A, B, N>::bar2;

    reuse Trait::<'a, 'b, 'c, A, B, N>::bar2::<i32, i32, i32, i32, i32, i32, 123, true> as bar3;
    //~^ ERROR: method takes 0 generic arguments but 8 generic arguments were supplied

    reuse Trait::<'a, 'b, 'c, A, B, N>::bar2::<i32, i32, i32, i32, i32, i32, 123, true> as bar4;
    //~^ ERROR: method `bar4` has 0 type parameters but its trait declaration has 3 type parameters
    //~| ERROR: method takes 0 generic arguments but 8 generic arguments were supplied
    //~| ERROR: generic arg X is not found in delegation
    //~| ERROR: generic arg Y is not found in delegation
    //~| ERROR: generic arg Z is not found in delegation
}


fn main() {
}
