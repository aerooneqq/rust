#![feature(fn_delegation)]

trait Trait: Sized {
    fn static_self() -> F { F }

    fn static_value(_: F) -> i32 { 1 }
    fn static_mut_ref(_: &mut F) -> i32 { 2 }
    fn static_ref(_: &F) -> i32 { 3 }
}

#[derive(Default, Eq, PartialEq, Debug)]
struct F;
impl Trait for F {}

struct S(F);

impl Trait for S {
    // Delegation's expression is removed from static functions.
    reuse <F as Trait>::* { self.0 }
    //~^ ERROR: target expression is specified in glob reuse where all functions are static
}

struct S1(F);
impl Trait for S1 {
    reuse <F as Trait>::{static_self} { self.0 }
}

struct S2(F);
impl Trait for S2 {
    reuse <F as Trait>::{static_self, static_value} { self.0 }
}

struct S3(F);
impl Trait for S3 {
    reuse <F as Trait>::{static_self, static_value, static_mut_ref, static_ref} { self.0 }
}

struct S4(F);
impl Trait for S4 {
    reuse <F as Trait>::{static_self, static_value, static_mut_ref, static_ref} { }
}

fn main() {}
