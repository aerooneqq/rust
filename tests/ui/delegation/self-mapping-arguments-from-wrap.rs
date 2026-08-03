//@ run-pass
//@ check-run-results

#![feature(fn_delegation)]

mod single_from {
    trait MyAdd {
        fn add(self, other: Self) -> Box<Self>;
    }

    impl MyAdd for usize {
        fn add(self, other: usize) -> Box<usize> {
            Box::new(self + other)
        }
    }

    #[derive(Eq, PartialEq, Debug)]
    struct W(Box<usize>);

    reuse impl MyAdd for W {
        println!("single_from {self:?}");
        *self.0
    }

    pub fn check() {
        fn w(x: usize) -> W {
            W(Box::new(x))
        }

        assert_eq!(w(1).add(w(2)), Box::new(w(3)))
    }
}

mod many_froms {
    use std::sync::Arc;
    use std::rc::Rc;

    trait MyAdd {
        fn add(self, other: Self) -> Box<Box<Box<Arc<Box<Rc<Self>>>>>>;
    }

    impl MyAdd for usize {
        fn add(self, other: usize) -> Box<Box<Box<Arc<Box<Rc<usize>>>>>> {
            Box::new(Box::new(Box::new(Arc::new(Box::new(Rc::new(self + other))))))
        }
    }

    #[derive(Eq, PartialEq, Debug)]
    struct W(Box<Box<Box<Arc<Box<Rc<usize>>>>>>);

    reuse impl MyAdd for W {
        println!("many_froms {self:?}");
        ******self.0
    }

    pub fn check() {
        fn w(x: usize) -> W {
            W(Box::new(Box::new(Box::new(Arc::new(Box::new(Rc::new(x)))))))
        }

        w(1).add(w(2));
    }
}

mod many_froms_2 {
    use std::sync::Arc;
    use std::rc::Rc;

    trait MyAdd {
        fn add(self, other: Self) -> Box<Arc<Rc<Box<Rc<Self>>>>>;
    }

    impl MyAdd for usize {
        fn add(self, other: usize) -> Box<Arc<Rc<Box<Rc<usize>>>>> {
            Box::new(Arc::new(Rc::new(Box::new(Rc::new(self + other)))))
        }
    }

    #[derive(Eq, PartialEq, Debug)]
    struct W(Box<Arc<Rc<Box<Rc<usize>>>>>);

    reuse impl MyAdd for W {
        println!("many_froms_2 {self:?}");
        *****self.0
    }

    pub fn check() {
        fn w(x: usize) -> W {
            W(Box::new(Arc::new(Rc::new(Box::new(Rc::new(x))))))
        }

        w(1).add(w(2));
    }
}

mod custom_froms {
    #[derive(Debug)]
    struct S1<A> {
        a: A,
    }

    impl<A> From<A> for S1<A> {
        fn from(a: A) -> S1<A> {
            S1 { a }
        }
    }

    #[derive(Debug)]
    struct S2<T> {
        t: T,
    }

    impl<T> From<T> for S2<T> {
        fn from(t: T) -> S2<T> {
            S2 { t }
        }
    }

    #[derive(Debug)]
    struct S3<'a, const C: usize, T> {
        t: T,
        pd: std::marker::PhantomData<&'a [usize; C]>
    }

    impl<'a, const C: usize, T> From<T> for S3<'a, C, T> {
        fn from(t: T) -> S3<'a, C, T> {
            S3 {
                t,
                pd: std::marker::PhantomData::<&'a [usize; C]>,
            }
        }
    }

    trait MyAdd: Sized {
        fn add(self, other: Self) -> S3<'static, 123, S2<S2<S1<usize>>>>;
    }

    impl MyAdd for usize {
        fn add(self, other: usize) -> S3<'static, 123, S2<S2<S1<usize>>>> {
            S3::from(S2::from(S2::from(S1::from(self + other))))
        }
    }

    #[derive(Debug)]
    struct W(S3<'static, 123, S2<S2<S1<usize>>>>);

    reuse impl MyAdd for W {
        println!("custom_froms {self:?}");
        self.0.t.t.t.a
    }

    pub fn check() {
        fn w(x: usize) -> W {
            W(S3::from(S2::from(S2::from(S1::from(x)))))
        }

        w(1).add(w(2));
    }
}

fn main() {
    single_from::check();
    many_froms::check();
    many_froms_2::check();
    custom_froms::check();
}
