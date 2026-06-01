pub fn generic_function<T, U>(x: T, y: U) -> T
where
    T: std::fmt::Display + Clone,
    U: std::fmt::Debug,
{
    println!("x = {}, y = {:?}", x, y);
    x.clone()
}

pub struct Wrapper<T, U, V> {
    pub first: T,
    pub second: U,
    pub third: V,
}

impl<T: Clone, U: Clone + std::fmt::Debug, V: std::fmt::Display> Wrapper<T, U, V> {
    pub fn new(first: T, second: U, third: V) -> Self {
        Self {
            first,
            second,
            third,
        }
    }

    pub fn display_all(&self)
    where
        T: std::fmt::Display,
    {
        println!("{} {:?} {}", self.first, self.second, self.third);
    }
}

pub trait MultiTrait<T, U> {
    fn do_something(&self, input: T) -> U;
    fn do_another(&self, a: T, b: U) -> (T, U);
}

pub struct ImplOne;

impl<T: Clone, U: Default> MultiTrait<T, U> for ImplOne {
    fn do_something(&self, input: T) -> U {
        U::default()
    }

    fn do_another(&self, a: T, b: U) -> (T, U) {
        (a, b)
    }
}

pub struct ImplTwo<'a, T: 'a + Clone, U: 'a + std::fmt::Debug> {
    pub reference: &'a T,
    pub data: U,
}

impl<'a, T: 'a + Clone, U: 'a + std::fmt::Debug> ImplTwo<'a, T, U> {
    pub fn new(reference: &'a T, data: U) -> Self {
        Self { reference, data }
    }

    pub fn get_reference(&self) -> &T {
        self.reference
    }

    pub fn get_data(&self) -> &U {
        &self.data
    }
}

pub fn lifetime_chaos<'a, 'b, 'c>(x: &'a str, y: &'b str, z: &'c str) -> String
where
    'a: 'b,
    'c: 'a,
{
    format!("{} {} {}", x, y, z)
}

pub fn complex_generic_return<T, U, V>(a: T, b: U, c: V) -> impl std::fmt::Display
where
    T: std::fmt::Display,
    U: std::fmt::Display,
    V: std::fmt::Display,
{
    format!("{} {} {}", a, b, c)
}

pub struct DeeplyNested<A, B, C, D, E, F> {
    pub a: A,
    pub b: B,
    pub c: C,
    pub d: D,
    pub e: E,
    pub f: F,
}

impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> DeeplyNested<A, B, C, D, E, F> {
    pub fn clone_all(&self) -> (A, B, C, D, E, F) {
        (
            self.a.clone(),
            self.b.clone(),
            self.c.clone(),
            self.d.clone(),
            self.e.clone(),
            self.f.clone(),
        )
    }
}

pub fn higher_ranked<F>(f: F)
where
    F: for<'a> Fn(&'a str) -> &'a str,
{
    let result = f("hello");
    println!("{}", result);
}

pub trait AssociatedTypes {
    type Output;
    type Error;

    fn process(&self) -> Result<Self::Output, Self::Error>;
}

pub struct Processor;

impl AssociatedTypes for Processor {
    type Output = String;
    type Error = i32;

    fn process(&self) -> Result<Self::Output, Self::Error> {
        Ok("done".to_string())
    }
}

pub fn generic_collections<T>(items: Vec<T>) -> Vec<T>
where
    T: std::cmp::PartialOrd + Clone,
{
    let mut items = items;
    items.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    items
}

pub fn extreme_bounds<T, U, V, W>(a: T, b: U, c: V, d: W) -> bool
where
    T: std::ops::Add<Output = T> + Into<U> + Clone,
    U: std::ops::Sub<Output = U> + std::fmt::Debug,
    V: std::cmp::PartialEq<V> + Default,
    W: std::iter::IntoIterator<Item = T>,
{
    let _cloned = a.clone();
    println!("b is {:?}", b);
    c == V::default()
}

pub struct ConstGeneric<const N: usize> {
    pub data: [i32; N],
}

impl<const N: usize> ConstGeneric<N> {
    pub fn sum(&self) -> i32 {
        self.data.iter().sum()
    }

    pub fn reverse(&self) -> [i32; N] {
        let mut arr = self.data;
        arr.reverse();
        arr
    }
}

pub fn where_clause_madness<T, U>(a: T, b: U) -> String
where
    T: std::fmt::Display,
    U: std::fmt::Display,
    T: Clone,
    U: Clone,
    String: From<T> + From<U>,
{
    let s1: String = a.into();
    let s2: String = b.into();
    format!("{}{}", s1, s2)
}

pub enum GenericEnum<T, U, V> {
    VariantA(T),
    VariantB(U, V),
    VariantC { x: T, y: U, z: V },
    VariantD,
}

pub fn handle_generic_enum<T, U, V>(val: GenericEnum<T, U, V>)
where
    T: std::fmt::Display,
    U: std::fmt::Debug,
    V: Default + std::fmt::Debug,
{
    match val {
        GenericEnum::VariantA(x) => println!("A: {}", x),
        GenericEnum::VariantB(u, v) => println!("B: {:?} {:?}", u, v),
        GenericEnum::VariantC { x: _, y: _, z } => {
            let _ = z;
        }
        GenericEnum::VariantD => {}
    }
}

pub fn multiple_where_clauses<T, U>(a: &T, b: &U) -> bool
where
    T: std::cmp::PartialEq<U>,
    T: std::fmt::Display,
    U: std::fmt::Display,
{
    println!("a = {}, b = {}", a, b);
    a == b
}

pub struct PhantomHolder<T> {
    pub value: u32,
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> PhantomHolder<T> {
    pub fn new(value: u32) -> Self {
        Self {
            value,
            _marker: std::marker::PhantomData,
        }
    }
}

pub fn dispatch_example(x: impl Into<String>) -> String {
    x.into()
}

pub fn impl_trait_args(a: impl std::fmt::Display, b: impl std::fmt::Debug) -> String {
    format!("display: {}, debug: {:?}", a, b)
}

pub struct DoubleEnded<'a, 'b, T: 'a + 'b + Clone, U: 'a + 'b> {
    pub left: &'a T,
    pub right: &'b U,
}

impl<'a, 'b, T: 'a + 'b + Clone, U: 'a + 'b> DoubleEnded<'a, 'b, T, U> {
    pub fn swap(&self) -> (&U, &T) {
        (self.right, self.left)
    }
}

pub fn type_alias_generics<T>(x: T) -> MyResult<T>
where
    T: std::fmt::Display,
{
    println!("got {}", x);
    Ok(x)
}

type MyResult<T> = Result<T, String>;

pub fn existential_impl(x: i32) -> impl std::ops::Add<i32, Output = impl std::fmt::Display> {
    x + 1
}

pub fn complex_return_type(x: i32) -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    Ok(Some(vec![x.to_string()]))
}

pub fn generic_closure<F>(f: F)
where
    F: Fn(i32) -> i32,
{
    println!("{}", f(42));
}

pub struct Builder<T, U, V, W> {
    pub step1: Option<T>,
    pub step2: Option<U>,
    pub step3: Option<V>,
    pub step4: Option<W>,
}

impl<T, U, V, W> Builder<T, U, V, W> {
    pub fn new() -> Self {
        Self {
            step1: None,
            step2: None,
            step3: None,
            step4: None,
        }
    }

    pub fn with_step1(mut self, val: T) -> Builder<T, U, V, W> {
        self.step1 = Some(val);
        self
    }

    pub fn with_step2(mut self, val: U) -> Builder<T, U, V, W> {
        self.step2 = Some(val);
        self
    }

    pub fn build(self) -> (Option<T>, Option<U>, Option<V>, Option<W>) {
        (self.step1, self.step2, self.step3, self.step4)
    }
}

pub fn turbofish_example() {
    let _x: Vec<i32> = "1,2,3"
        .split(",")
        .map(|s| s.parse::<i32>().unwrap())
        .collect();
    let _y = "hello".parse::<String>().unwrap();
}

pub fn huge_generic_signature<
    T: Clone + std::fmt::Debug + std::fmt::Display + Default + PartialEq,
    U: Clone + std::fmt::Debug + std::fmt::Display + Default + PartialEq + std::hash::Hash,
    V: Clone + std::fmt::Debug + std::fmt::Display + Default,
>(
    a: &T,
    b: &U,
    c: &V,
) -> (T, U, V) {
    (a.clone(), b.clone(), c.clone())
}

pub trait SuperTrait<T>: MultiTrait<T, T> + std::fmt::Debug
where
    T: Clone + Default,
{
    fn extra_method(&self) -> T;
}

#[derive(Debug)]
pub struct SuperImpl;

impl<T: Clone + Default + std::fmt::Debug> SuperTrait<T> for SuperImpl {
    fn extra_method(&self) -> T {
        T::default()
    }
}

impl<T: Clone + Default + std::fmt::Debug> MultiTrait<T, T> for SuperImpl {
    fn do_something(&self, input: T) -> T {
        input
    }

    fn do_another(&self, a: T, b: T) -> (T, T) {
        (a, b)
    }
}

pub fn default_type_param<T>(x: T) -> T {
    x
}

pub fn const_eval<const N: usize>() -> [i32; N] {
    [0; N]
}

pub fn zst_generics<T>() -> usize
where
    T: Sized,
{
    std::mem::size_of::<T>()
}
