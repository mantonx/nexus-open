package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"nexus-open/nexus"
)

func main() {
	// Set up graceful shutdown
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	// Start the Nexus backend
	go nexus.StartNexus()

	// Wait for shutdown signal
	<-ctx.Done()
	log.Println("Shutting down gracefully...")

	// TODO: Implement proper shutdown in nexus.StopNexus()
	// nexus.StopNexus()
}
