This is a simple client-server experiment, where the server holds a hash table and the client can enqueue operations for the server to perform.

The communication between the client and the server happens via a message queue located in POSIX shared memory.

Server:

The server keeps a hash table, storing key-value pairs (both <code>usize</code> in this example), in which collisions are avoided using buckets. The amount of buckets in the table is asked from the user at startup. To make multithreaded operations on the hash table possible, each bucket possesses a read-write lock. The server creates two shared memory regions, one to receive messages and one to send back responses. It then spawns a worker thread to dequeue incoming messages and perform the specified operations. If an operation requires a response, e.g., the <code>Read(key)</code> operation, it will enqueue the response in the outgoing message queue. The client can request n additional worker threads by sending <code>ThStart(n)</code> message or stop a thread by sending the <code>ThStop</code> message. The thread scaling is limited by the message queue, however, as it bottlenecks the speed at which new messages can be picked up.The server supports the following operations:
<code>Insert(key, value), Read(key), Delete(key), Print(index), ThStart(n), ThStop, Quit
</code>

Client:

The client has to be run after the server and first connects to the message queues set up by the server. It then starts a thread to buffer incoming responses to avoid a situation where the main client thread waits on a response and the server simultaneously waits on further instructions. The main thread then starts to generate test data and runs multiple test routines to test the server's performance. Expected responses are taken from the aforementioned buffer and evaluated for any errors.
Each routine is run 5 times with the number of requested worker threads doubling each run and a rough measurement of the time taken is provided. 

Known problems:

Bad scalability:
    Despite the multithreading capabilities of the server, the whole setup is fundamentally limited by the single-threaded nature of the client. The messages can not be enqueued fast enough so that all server threads are working at full capacity. Therefore, there is no speedup or even slowdown for more than 4 threads, as the threads are only waiting for work and blocking each other.
    A potential solution could be to design a multi-threaded client.

Sequential reads: 
    As every read has to wait for a response from the server via the message queue, sequential reads can only be enqueued after the response to the previous read is received. This severely limits the speed at which sequential reads can be processed. 
    A potential solution could be asynchronous communication using some sort of ID system.

How to test: 

1. compile by running: 
> cargo build
1. start the server by running: 
> cargo run --bin server
1. Enter the desired number of buckets for the hash table (default: 5000). Small values severely impact performance if the number of inserted items is large.
2. run the client by running: <console>
> cargo run --bin client </console>
>
1. (Optional): Change the <code>client.rs</code> file to adjust the test parameters or create new tests. 