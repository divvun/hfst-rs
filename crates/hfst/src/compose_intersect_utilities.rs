//! Port of 'libhfst/src/implementations/compose_intersect/ComposeIntersectUtilities.{h,cc}'.
//!
//! Defines the generic, space-saving sorted-vector set 'SpaceSavingSet<X, C>'
//! used by the 'compose_intersect' machinery, together with the 'int'
//! comparator ('CmpInt') and the concrete 'IntSpaceSavingSet' instantiation
//! from the '.cc' file.
//!
//! 1:1 literal C++ -> Rust translation: the C++ template parameter 'C' (a
//! comparator *class* whose 'operator()(const X&, const X&)' yields a strict
//! less-than) is modelled here as the ['Comparator'] trait. In C++ the
//! comparator was held as a static member 'static C comparator;'
//! (default-constructed); here the comparison is provided through the trait's
//! associated function, which is the faithful equivalent of calling that
//! default-constructed functor.

use std::marker::PhantomData;

/// Comparator class, mirroring the C++ template parameter 'C'.
///
/// The C++ code uses 'comparator(*it, x)' where 'comparator' is a
/// default-constructed static instance of 'C' with
/// 'bool operator()(const X &, const X &) const'. The trait method below is the
/// 1:1 equivalent of that functor call.
pub trait Comparator<X> {
    // [spec:hfst:sem:compose-intersect-utilities.cmp-int.operator-fn]
    fn compare(x1: &X, x2: &X) -> bool;
}

/// 'std::vector<X>'.
// [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.x-vector]
pub type XVector<X> = Vec<X>;

// [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set]
pub struct SpaceSavingSet<X, C>
where
    C: Comparator<X>,
{
    container: XVector<X>,
    // C++ carries 'static C comparator;' and 'static ReverseCompare reverse_comp;'
    // as static members; in Rust the comparator lives in the type parameter 'C',
    // so we only need a phantom to retain it.
    _comparator: PhantomData<C>,
}

// [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.const-iterator]
// [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.iterator]
// const_iterator / iterator are 'XVector::const_iterator' / 'XVector::iterator';
// in Rust these are slice iterators (see the begin/end methods below). The C++
// index-by-iterator used in 'add_value'/'insert' is represented here as a 'usize'
// position into 'container'.
pub type ConstIterator<'a, X> = std::slice::Iter<'a, X>;
pub type Iterator<'a, X> = std::slice::IterMut<'a, X>;

impl<X, C> SpaceSavingSet<X, C>
where
    X: PartialEq,
    C: Comparator<X>,
{
    pub fn new() -> Self {
        SpaceSavingSet {
            container: XVector::new(),
            _comparator: PhantomData,
        }
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.begin-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.begin-fn]
    pub fn begin(&self) -> ConstIterator<'_, X> {
        self.container.iter()
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.end-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.end-fn]
    pub fn end(&self) -> ConstIterator<'_, X> {
        self.container[self.container.len()..].iter()
    }

    pub fn begin_mut(&mut self) -> Iterator<'_, X> {
        self.container.iter_mut()
    }

    pub fn end_mut(&mut self) -> Iterator<'_, X> {
        let len = self.container.len();
        self.container[len..].iter_mut()
    }

    // SpaceSavingSet &operator=(const SpaceSavingSet &another)
    pub fn assign(&mut self, another: &SpaceSavingSet<X, C>) -> &mut Self
    where
        X: Clone,
    {
        self.container = another.container.clone();
        self
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.insert-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.insert-fn]
    pub fn insert(&mut self, x: &X)
    where
        X: Clone,
    {
        // 'least_upper_bound' is an index position into 'container'; the C++
        //   'iterator least_upper_bound = get_least_upper_bound(x);'
        // yields the position of the least element not less than 'x' (== end()
        // when all elements are less than 'x').
        let least_upper_bound = self.get_least_upper_bound_index(x);
        // C++:
        //   const X &new_x = *least_upper_bound;
        //   if (least_upper_bound == end() || !(x == new_x))
        //     { add_value(x,least_upper_bound); }
        // The reference 'new_x' is formed by dereferencing the iterator even when
        // it equals end() (UB in C++), but it is only read in the '!(x == new_x)'
        // branch, which is short-circuited away when 'least_upper_bound == end()'.
        // We reproduce the observable behaviour by guarding the dereference.
        let at_end = least_upper_bound == self.container.len();
        if at_end || !(*x == self.container[least_upper_bound]) {
            self.add_value(x, least_upper_bound);
        }
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.find-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.find-fn]
    pub fn find(&self, x: &X) -> ConstIterator<'_, X> {
        let least_upper_bound = self.get_least_upper_bound_index(x);
        if least_upper_bound == self.container.len() {
            return self.end();
        }

        let new_x = &self.container[least_upper_bound];
        if *new_x != *x {
            return self.end();
        }

        self.container[least_upper_bound..].iter()
    }

    /// Index-based equivalent of 'find', returning the matched position or
    /// 'container.len()' (the end() sentinel) when not present. Provided so
    /// callers can mirror C++ iterator comparisons ('find(x) != end()') without
    /// juggling slice iterators.
    pub fn find_index(&self, x: &X) -> usize {
        let least_upper_bound = self.get_least_upper_bound_index(x);
        if least_upper_bound == self.container.len() {
            return self.container.len();
        }

        let new_x = &self.container[least_upper_bound];
        if *new_x != *x {
            return self.container.len();
        }

        least_upper_bound
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.clear-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.clear-fn]
    pub fn clear(&mut self) {
        self.container.clear();
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.has-element-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.has-element-fn]
    pub fn has_element(&self, x: &X) -> bool {
        // C++: find(x) != end()
        self.find_index(x) != self.container.len()
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.size-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.size-fn]
    pub fn size(&self) -> usize {
        self.container.len()
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.get-least-upper-bound-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.get-least-upper-bound-fn]
    //
    // C++ had two overloads (const_iterator / iterator) that share a body. Here
    // a single index-returning helper covers both; 'container.len()' is the
    // end() sentinel.
    fn get_least_upper_bound_index(&self, x: &X) -> usize {
        let mut it = 0usize;
        while it != self.container.len() {
            if !C::compare(&self.container[it], x) {
                break;
            }
            it += 1;
        }
        it
    }

    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.add-value-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.add-value-fn]
    fn add_value(&mut self, x: &X, least_upper_bound: usize)
    where
        X: Clone,
    {
        // C++: container.insert(least_upper_bound, x);
        self.container.insert(least_upper_bound, x.clone());
    }
}

impl<X, C> Default for SpaceSavingSet<X, C>
where
    X: PartialEq,
    C: Comparator<X>,
{
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare]
// 'static struct ReverseCompare { ... } reverse_comp;'
//
// This nested functor is declared (and statically instantiated as 'reverse_comp')
// in the C++ header but is never referenced by any live code path in
// 'SpaceSavingSet'. It is carried here for fidelity. Its 'operator()' body —
// 'return comparator()(x1, x2);' — likewise has no caller.
pub struct ReverseCompare<X, C>(PhantomData<(X, C)>)
where
    C: Comparator<X>;

impl<X, C> ReverseCompare<X, C>
where
    C: Comparator<X>,
{
    // [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare.operator-fn]
    // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare.operator-fn]
    pub fn call(x1: &X, x2: &X) -> bool {
        // C++: return comparator()(x1,x2);
        C::compare(x1, x2)
    }
}

// ---------------------------------------------------------------------------
// From ComposeIntersectUtilities.cc
// ---------------------------------------------------------------------------

// [spec:hfst:def:compose-intersect-utilities.cmp-int]
pub struct CmpInt;

impl Comparator<i32> for CmpInt {
    // [spec:hfst:sem:compose-intersect-utilities.cmp-int.operator-fn]
    // [spec:hfst:def:compose-intersect-utilities.cmp-int.operator-fn]
    fn compare(i: &i32, j: &i32) -> bool {
        *i < *j
    }
}

// [spec:hfst:def:compose-intersect-utilities.int-space-saving-set]
//
// C++: typedef SpaceSavingSet<int,CmpInt> IntSpaceSavingSet;
//      template<> CmpInt IntSpaceSavingSet::comparator = CmpInt();
// The explicit static-member specialisation has no separate representation in
// Rust; the comparator is carried by the 'CmpInt' type parameter / trait impl.
pub type IntSpaceSavingSet = SpaceSavingSet<i32, CmpInt>;
