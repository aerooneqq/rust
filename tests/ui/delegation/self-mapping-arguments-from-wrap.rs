//@ check-pass

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
        println!("{self:?}");
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
        println!("{self:?}");
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
        println!("{self:?}");
        *****self.0
    }

    pub fn check() {
        fn w(x: usize) -> W {
            W(Box::new(Arc::new(Rc::new(Box::new(Rc::new(x))))))
        }

        w(1).add(w(2));
    }
}

fn main() {
}
