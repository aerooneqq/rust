#![feature(fn_delegation)]

trait Trait<'a, T, const F: fn(&CStr) -> usize> {
    //~^ ERROR: cannot find type `CStr` in this scope
    //~| ERROR: using function pointers as const generic parameters is forbidden
    fn foo<'x: 'x, A, B>(&self) {}
}

reuse Trait::foo;
//~^ ERROR: using function pointers as const generic parameters is forbidden
//~| WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
//~| WARN: this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!
//~| WARN: cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
//~| WARN: this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!

fn main() {}
