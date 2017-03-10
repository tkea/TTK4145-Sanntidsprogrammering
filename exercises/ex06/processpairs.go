package main

import (
    "net"
    "time"
    "encoding/binary"
    "fmt"
    "log"
    "os"
    "os/exec"
    "syscall"
)

var isPrimary bool // State

type AliveMessage struct {}

type CheckpointMessage struct {
    number uint64
}

// Go's UDP lib sucks. Here is a function that works
// via https://github.com/TTK4145/Network-go/blob/master/network/conn/bcast_conn.go
func DialBroadcastUDP(port int) net.PacketConn {
	s, _ := syscall.Socket(syscall.AF_INET, syscall.SOCK_DGRAM, syscall.IPPROTO_UDP)
	syscall.SetsockoptInt(s, syscall.SOL_SOCKET, syscall.SO_REUSEADDR, 1)
	syscall.SetsockoptInt(s, syscall.SOL_SOCKET, syscall.SO_BROADCAST, 1)
	syscall.Bind(s, &syscall.SockaddrInet4{Port: port})

	f := os.NewFile(uintptr(s), "")
	conn, _ := net.FilePacketConn(f)
	f.Close()

	return conn
}


func checkError(err error)  {
    if err != nil {
        log.Fatal(err) // NB: This calls os.Exit(1)
    }
}

func aliveMessageListener(aliveMessages chan AliveMessage)  {
    conn := DialBroadcastUDP(9090)

    for !isPrimary {
        var buf [1024]byte
        conn.ReadFrom(buf[0:])
        aliveMessages <- AliveMessage {}
    }

    conn.Close()
}

func checkpointMessageListener(checkpoints chan CheckpointMessage)  {
    conn := DialBroadcastUDP(9191)

    for !isPrimary {
        var buf [1024]byte
        n, _, _ := conn.ReadFrom(buf[0:])

        checkpoint, _ := binary.Uvarint(buf[0:n])

        msg := CheckpointMessage {checkpoint}
        checkpoints <- msg
    }

    conn.Close()
}

func aliveMessageSender()  {
    port := 9090
    addr, _ := net.ResolveUDPAddr("udp4", fmt.Sprintf("255.255.255.255:%d", port))

    conn := DialBroadcastUDP(port)

    for isPrimary {
        conn.WriteTo([]byte("I am alive."), addr)
        time.Sleep(10 * time.Millisecond)
    }

    conn.Close()
}

func checkpointMessageSender(checkpoints chan CheckpointMessage)  {
    port := 9191
    addr, _ := net.ResolveUDPAddr("udp4", fmt.Sprintf("255.255.255.255:%d", port))

    conn := DialBroadcastUDP(port)

    for isPrimary {
        checkpointData := <- checkpoints
        var checkpoint uint64

        buf := make([]byte, 15)
        binary.PutUvarint(buf, checkpoint)

        conn.WriteTo(buf, addr)
    }

    conn.Close()
}

func doWork(checkpoint uint64) {
    // The "work" to be done on primary
    //fmt.Println(checkpoint)
}

func primary(initialCheckpoint uint64)  {
    isPrimary = true // Update state

    log.Print("Primary started with initial checkpoint ", initialCheckpoint)

    checkpoint := initialCheckpoint // Possibly a duplicate

    // Spawn message sending goroutines
    checkpointTx := make(chan CheckpointMessage)

    go aliveMessageSender()
    go checkpointMessageSender(checkpointTx)

    // Spawn backup process
    cmd := exec.Command("gnome-terminal", "-x", "sh", "-c", "go run processpairs.go")
    cmd.Run()

    // Do the actual work and broadcast the new checkpoint
    for {
        doWork(checkpoint)
        // Now we have done the work, so broadcast the checkpoint
        checkpointTx <- CheckpointMessage {checkpoint}

        checkpoint++
    }
}

func backup() uint64 {
    // Read checkpoint & alive messages until last alive message is too old

    isPrimary = false // Update global state

    // State
    var checkpoint uint64

    // Spawn message listener goroutines
    aliveMessages := make(chan AliveMessage)
    checkpointMessages := make(chan CheckpointMessage)

    go aliveMessageListener(aliveMessages)
    go checkpointMessageListener(checkpointMessages)

    // Select on channels until primary stops sending alive messages
    // i.e. until last alive message is too old
    for {
        select {
        case <- aliveMessages:
            break

        case checkpointMsg := <- checkpointMessages:
            // Update state
            checkpoint = checkpointMsg.number
            log.Print("Checkpoint is now ", checkpoint)

        case <- time.After(150 * time.Millisecond):
            log.Print("Primary process timed out.")
            return checkpoint
        }
    }
}

func main()  {
    // Try being a backup first.
    // If no alive messages are received, or the current primary
    // crashes/times out, we'll become the new primary
    // and spawn a new backup process.
    checkpoint := backup()
    log.Print("Switching from backup mode to primary mode.")
    primary(checkpoint)
}
