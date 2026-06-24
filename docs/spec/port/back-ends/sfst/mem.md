# back-ends/sfst/mem.h

> [spec:hfst:def:mem.sfst.mem]
> class Mem {
>   struct MemBuffer { char buffer[MEMBUFFER_SIZE]; struct MemBuffer *next; };
>   MemBuffer *first_buffer;
>   long pos;
> }

> [spec:hfst:def:mem.sfst.mem.add-buffer-fn]
> void add_buffer()

> [spec:hfst:sem:mem.sfst.mem.add-buffer-fn]
> Allocates one new `MemBuffer` via `malloc(sizeof(MemBuffer))` (i.e. a buffer
> of `MEMBUFFER_SIZE` (100000) bytes plus a `next` pointer). If `malloc` returns
> `NULL`, throws the C-string `"Allocation of memory failed in Mem::add_buffer!"`.
> Otherwise prepends the new buffer onto the singly linked list: sets
> `mb->next = first_buffer`, then `first_buffer = mb` (the new buffer becomes the
> head). Resets `pos` to 0 so subsequent allocations start at the beginning of
> the new head buffer. Returns nothing.

> [spec:hfst:def:mem.sfst.mem.alloc-fn]
> void *alloc( size_t n )

> [spec:hfst:sem:mem.sfst.mem.alloc-fn]
> Bump-pointer allocator returning a `void*` to `n` bytes inside the current head
> buffer. First rounds `n` up to a multiple of 4 for alignment: if `n % 4 != 0`,
> adds `4 - (n % 4)` to `n`. Then, if `first_buffer == NULL` OR `pos + n >
> MEMBUFFER_SIZE` (the request would not fit in the remaining space of the head
> buffer), calls `add_buffer()` to prepend a fresh empty buffer and reset `pos`
> to 0. After that, if `pos + n > MEMBUFFER_SIZE` still holds (the request is
> larger than an entire fresh buffer), throws the C-string `"Allocation of memory
> block larger than MEMBUFFER_SIZE attempted!"`. Otherwise computes `result =
> first_buffer->buffer + pos`, advances `pos` by `n`, and returns `result`.
> Memory is never individually freed; only the whole arena is freed by `clear()`.

> [spec:hfst:def:mem.sfst.mem.clear-fn]
> void clear()

> [spec:hfst:sem:mem.sfst.mem.clear-fn]
> Frees the entire buffer list. Loops while `first_buffer != NULL`: saves
> `next = first_buffer->next`, calls `free(first_buffer)`, then sets
> `first_buffer = next`. After the loop `first_buffer` is `NULL`. Finally sets
> `pos = 0`. Returns nothing. This is also invoked by the destructor `~Mem()`.

> [spec:hfst:def:mem.sfst.mem.mem-buffer]
> struct MemBuffer {
>   char buffer[MEMBUFFER_SIZE];
>   struct MemBuffer *next;
> }

> [spec:hfst:def:mem.sfst.mem.mem-fn]
> Mem()

> [spec:hfst:sem:mem.sfst.mem.mem-fn]
> Constructor. Initializes `first_buffer = NULL`, then calls `add_buffer()`,
> which allocates the first `MemBuffer`, makes it the head of the list, and sets
> `pos = 0`. After construction the arena holds exactly one empty buffer ready
> for allocation.

