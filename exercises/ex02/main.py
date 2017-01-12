import threading

i = 0
lock = threading.Lock()


def increment_i():
    global i
    for _ in range(1000*1000):
        with lock:
            i = i+1


def decrement_i():
    global i
    for _ in range(1000*1000):
        with lock:
            i = i-1


def main():
    # Configure threads
    increment_thread = threading.Thread(target=increment_i)
    decrement_thread = threading.Thread(target=decrement_i)

    # Start threads
    increment_thread.start()
    decrement_thread.start()

    # Wait for threads to terminate
    increment_thread.join()
    decrement_thread.join()

    # Print i
    global i
    print(i)


main()
