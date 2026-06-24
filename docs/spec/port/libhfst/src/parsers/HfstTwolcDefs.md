# libhfst/src/parsers/HfstTwolcDefs.h

> [spec:hfst:def:hfst-twolc-defs.handy-deque]
> class HandyDeque : public std::deque<C>

> [spec:hfst:def:hfst-twolc-defs.handy-deque.get-front-and-pop-fn]
> C get_front_and_pop(void)

> [spec:hfst:sem:hfst-twolc-defs.handy-deque.get-front-and-pop-fn]
> Member of HandyDeque<C> (subclass of std::deque<C>). Reads the front element
> of the deque (copying it into a local temporary `temp`), removes the front
> element from the deque via pop_front, then returns the saved copy `temp`.
> Mutates the deque by shrinking it from the front by one. Precondition: the
> deque is non-empty (calling on an empty deque is undefined behavior, the same
> as calling front()/pop_front() on an empty std::deque). Returns the popped
> element by value.

> [spec:hfst:def:hfst-twolc-defs.handy-map]
> class HandyMap : public std::map<K,V>

> [spec:hfst:def:hfst-twolc-defs.handy-map.get-value-fn]
> V get_value(const K &k) const

> [spec:hfst:sem:hfst-twolc-defs.handy-map.get-value-fn]
> Const member of HandyMap<K,V> (subclass of std::map<K,V>). Looks up key `k`
> via std::map::find and returns the associated value (the `second` of the
> iterator's pair) by value. Does not mutate the map. Precondition: the key `k`
> exists in the map; if it does not, find returns end() and dereferencing it is
> undefined behavior.

> [spec:hfst:def:hfst-twolc-defs.handy-map.has-key-fn]
> bool has_key(const K &k) const

> [spec:hfst:sem:hfst-twolc-defs.handy-map.has-key-fn]
> Const member of HandyMap<K,V> (subclass of std::map<K,V>). Calls
> std::map::find(k) and returns true if the result is not end() (i.e. the key
> `k` is present in the map), false otherwise. Does not mutate the map.

> [spec:hfst:def:hfst-twolc-defs.handy-set]
> class HandySet : public std::set<V>

> [spec:hfst:def:hfst-twolc-defs.handy-set.has-element-fn]
> bool has_element(const V &v) const

> [spec:hfst:sem:hfst-twolc-defs.handy-set.has-element-fn]
> Const member of HandySet<V> (subclass of std::set<V>). Calls
> std::set::find(v) and returns true if the result is not end() (i.e. the
> element `v` belongs to the set), false otherwise. Does not mutate the set.

