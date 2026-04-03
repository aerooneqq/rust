#![feature(fn_delegation)]
#![allow(incomplete_features)]

trait Trait {
    fn method(&self);
    const CONST: u8; //~ ERROR: failed to resolve delegation
    type Type; //~ ERROR: failed to resolve delegation
    #[allow(non_camel_case_types)]
    type method; //~ ERROR: failed to resolve delegation
}

impl Trait for u8 {
    fn method(&self) {}
    const CONST: u8 = 0;
    type Type = u8;
    type method = u8;
}

struct Good(u8);
impl Trait for Good {
    reuse Trait::* { &self.0 }
    // Explicit definitions for non-delegatable items.
    const CONST: u8 = 0;
    type Type = u8;
    type method = u8;
}

struct Bad(u8);
impl Trait for Bad { //~ ERROR not all trait items implemented, missing: `CONST`, `Type`, `method`
    reuse Trait::* { &self.0 }
    //~^ ERROR item `CONST` is an associated method, which doesn't match its trait `Trait`
    //~| ERROR item `Type` is an associated method, which doesn't match its trait `Trait`
    //~| ERROR duplicate definitions with name `method`
    //~| ERROR expected function, found associated constant `Trait::CONST`
}

fn main() {}
