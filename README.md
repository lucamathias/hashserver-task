This is a simple client-server experiment, where the server holds a hash table and the client can enqueue operations for the server to perform.

The hash table holds key-value pairs (both integers in this example) and uses buckets to resolve hash collisions. The buckets are protected by read-write locks, making the hash table thread-safe. It supports the operations insert, delete, read and print. Duplicate keys are allowed, but the delete operation will remove all key-value pairs with a given key. 

How it works:

The communication between the client and the server happens via POSIX shared memory in a n-to-n thread model. Each client thread communicates directly with one server thread through a shared slot in memory. The client writes operations to memory and the server reads and replaces them with the appropriate response (if a response is needed). The slot of memory is protected by a mutex to prevent race conditions and the server thread waits passively on the client thread to notify it about new work. To preserve the sequence of operations, each operation is delivered together with a sequence number. This allows for concurrent execution of operations as long as all operations with lower sequence numbers have already acquired the lock for the bucket of the hash table they will be operating on.

How to test: 

1. Run the server executable and enter the desired amount of buckets for the hash table (default: 5000). A much lower number will severely limit performance with large amounts of inserted items.
2. Run the client executable, it contains some tests that will run, print rough time measurements and then stop the server.
3. New tests can be added to the client.rs file and the amount of threads to use can be adjusted in the util.rs file.

Evaluation Numbers:

The machine used in this test had 8 cores with 16 threads, as both the client and the server run a thread, a test with the number of threads set to 8 uses all available cores on the machine and therefore does perform worse due to conflicts with other computing load. All tests were conducted using the default number of 5000 buckets for the hash table.

Test:\
 Inserting 1.000.000 sequential values, read them to check correct insertion, delete them and read them again to check correct deletion.

1 thread: 8.08s \
2 threads: 4.26s \
4 threads: 1.84s \
6 threads: 1.30s \
8 threads: 1.26s 

Test: \
Inserting 1.000.000 random values, delete 10 of them randomly, check that the deletion was successful and then check if all other values are still present.

1 thread: 4.49s \
2 threads: 2.60s \
4 threads: 1.27s \
6 threads: 0.95s \
8 threads: 0.71s 

