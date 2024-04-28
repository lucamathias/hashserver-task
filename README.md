This is a simple client-server experiment, where the server holds a hash table and the client can enqueue operations for the server to perform.

The hash table holds key-value pairs (both integers in this example) and uses buckets to resolve hash collisions. The buckets are protected by read-write locks, making the hash table thread-safe. It supports the operations insert, delete, read and print.

The communication between the client and the server happens via POSIX shared memory in a n-to-n thread model. Each client thread communicates directly with one server thread through a shared slot in memory. The client writes operations to memory and the server reads and replaces them with the appropriate response (if a response is needed). The slot of memory is protected by a mutex to prevent race conditions.

