#![feature(fn_delegation)]
#![allow(incomplete_features)]

trait Bound<T> {}

trait Trait<'a, T, const X: usize>
where
    Self: Bound<T>,
{
    fn static_method<'c: 'c, U, const B: bool>() {}
}

impl<'a, T, const X: usize> Trait<'a, T, X> for usize {}
impl<T> Bound<T> for usize {}

reuse <usize as Trait<'static, i32, 123>>::static_method as foo;
reuse <usize as Trait<'static, i32, 123>>::static_method::<String, false> as foo2;
reuse <usize as Trait>::static_method as bar;
reuse <usize as Trait>::static_method::<Vec<i32>, false> as bar2;

reuse Trait::static_method as error;
//~^ ERROR: type annotations needed
reuse Trait::<'static, i32, 123>::static_method as error2;
//~^ ERROR: type annotations needed
reuse Trait::<'static, i32, 123>::static_method::<'static, String, false> as error3;
//~^ ERROR: type annotations needed
reuse Trait::static_method::<'static, Vec<i32>, false> as error4;
//~^ ERROR: type annotations needed

reuse <String as Trait>::static_method as error5;
//~^ ERROR: the trait bound `String: Trait<'a, T, X>` is not satisfied

struct Struct;
impl<'a, T, const X: usize> Trait<'a, T, X> for Struct {}
//~^ ERROR: the trait bound `Struct: Bound<T>` is not satisfied

reuse <Struct as Trait>::static_method as error6;
//~^ ERROR: the trait bound `Struct: Bound<T>` is not satisfied

pub fn check<'a>(s: &'a str) {
    foo::<'a, String, true>();
    foo2();

    bar::<'static, 'a, i32, 123, String, false>();
    bar2::<'static, usize, 321>();
}

fn main() {
    check("");
}
