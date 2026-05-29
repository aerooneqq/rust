#![feature(fn_delegation)]

pub trait Trait: Sized {
    fn static_self() -> F { F }

    fn static_value(_: Self) -> i32 { 1 }
    fn static_mut_ref(_: &mut Self) -> i32 { 2 }
    fn static_ref(_: &Self) -> i32 { 3 }
}

struct F;
impl Trait for F {}

struct S(F);

reuse impl Trait for S {
    //~^ ERROR: attempted to delete delegation's block that contains definitions inside
    //~| ERROR: attempted to delete delegation's block that contains definitions inside
    //~| ERROR: attempted to delete delegation's block that contains definitions inside
    //~| ERROR: attempted to delete delegation's block that contains definitions inside
    //~| ERROR: mismatched types
    //~| ERROR: mismatched types
    //~| ERROR: mismatched types
    //~| ERROR: mismatched types
    //~| ERROR: method `static_self` has an incompatible type for trait
    //~| ERROR: method `static_value` has 0 parameters but the declaration in trait `Trait::static_value` has 1
    //~| ERROR: method `static_mut_ref` has 0 parameters but the declaration in trait `Trait::static_mut_ref` has 1
    //~| ERROR: method `static_ref` has 0 parameters but the declaration in trait `Trait::static_ref` has 1
    //~| ERROR: this function takes 0 arguments but 1 argument was supplied
    struct Def {}

    self
}

fn main() {}
