// Package api provides the HTTP API server for configuration and image management.
package api

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"nexus-open/internal/config"
)

// Server manages the HTTP API server.
type Server struct {
	server *http.Server
	logger *slog.Logger
	cfg    *config.Manager
}

// NewServer creates a new API server instance.
func NewServer(addr string, cfg *config.Manager, logger *slog.Logger) *Server {
	if logger == nil {
		logger = slog.Default()
	}

	s := &Server{
		logger: logger,
		cfg:    cfg,
	}

	// Create router
	mux := http.NewServeMux()
	s.registerRoutes(mux)

	// Create HTTP server with timeouts
	s.server = &http.Server{
		Addr:         addr,
		Handler:      s.middleware(mux),
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	return s
}

// Start begins listening for HTTP requests.
func (s *Server) Start() error {
	s.logger.Info("starting API server", "addr", s.server.Addr)

	if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		return fmt.Errorf("server error: %w", err)
	}

	return nil
}

// Shutdown gracefully stops the API server.
func (s *Server) Shutdown(ctx context.Context) error {
	s.logger.Info("shutting down API server")

	if err := s.server.Shutdown(ctx); err != nil {
		return fmt.Errorf("server shutdown error: %w", err)
	}

	return nil
}

// registerRoutes sets up all API endpoints.
func (s *Server) registerRoutes(mux *http.ServeMux) {
	mux.HandleFunc("/api/health", s.handleHealth)
	mux.HandleFunc("/api/config", s.handleConfig)
	mux.HandleFunc("/api/images/upload", s.handleImageUpload)
	mux.HandleFunc("/api/images", s.handleListImages)
	mux.HandleFunc("/api/images/delete", s.handleDeleteImage)
}
