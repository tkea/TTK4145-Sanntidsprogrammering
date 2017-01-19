package main

import (
    "fmt"
    "runtime"
    "time"
)

const NUM_COMPUTATIONS int = 1000*1000


func increment_i(increment_channel chan int) {
    i := <-increment_channel

    for x := 0; x < NUM_COMPUTATIONS; x++ {
        i++
    }

    increment_channel <- i
}

func decrement_i(increment_channel chan int) {
    i := <-increment_channel

    for x := 0; x < NUM_COMPUTATIONS; x++ {
        i--
    }

    increment_channel <- i
}

func main() {
    runtime.GOMAXPROCS(runtime.NumCPU())

    increment_channel := make(chan int, 1)
    increment_channel <- 0

    go increment_i(increment_channel)
    go decrement_i(increment_channel)

    time.Sleep(100*time.Millisecond)

    i := <-increment_channel
    fmt.Println(i)
}
