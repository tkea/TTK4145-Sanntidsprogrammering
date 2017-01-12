package main

import (
    . "fmt"
    "runtime"
    "time"
)

const NUM_COMPUTATIONS int = 1000*1000

var i int

func increment_i() {
    for x := 0; x < NUM_COMPUTATIONS; x++ {
        i++
    }
}

func decrement_i() {
    for x := 0; x < NUM_COMPUTATIONS; x++ {
        i--
    }
}

func main() {
    runtime.GOMAXPROCS(runtime.NumCPU())

    go increment_i()
    go decrement_i()

    time.Sleep(100*time.Millisecond)
    Println(i)
}
