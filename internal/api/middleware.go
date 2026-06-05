package api

import (
	"bufio"
	"log/slog"
	"net"
	"net/http"
	"time"
)

// middleware applies all middleware to the handler chain.
func (s *Server) middleware(next http.Handler) http.Handler {
	// Apply middleware in order: logging -> CORS -> handler
	return s.loggingMiddleware(s.corsMiddleware(next))
}

// corsMiddleware adds CORS headers to all responses.
func (s *Server) corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Allow all origins for development
		// TODO: Make this configurable for production
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		w.Header().Set("Access-Control-Max-Age", "3600")

		// Handle preflight requests
		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		next.ServeHTTP(w, r)
	})
}

// loggingMiddleware logs all HTTP requests.
func (s *Server) loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()

		// Wrap response writer to capture status code
		wrapped := &responseWriter{
			ResponseWriter: w,
			statusCode:     http.StatusOK,
		}

		// Call next handler
		next.ServeHTTP(wrapped, r)

		// Log request
		duration := time.Since(start)
		s.logger.Info("http request",
			"method", r.Method,
			"path", r.URL.Path,
			"status", wrapped.statusCode,
			"duration_ms", duration.Milliseconds(),
			"remote_addr", r.RemoteAddr,
		)
	})
}

// responseWriter wraps http.ResponseWriter to capture status code.
type responseWriter struct {
	http.ResponseWriter
	statusCode int
}

func (rw *responseWriter) WriteHeader(code int) {
	rw.statusCode = code
	rw.ResponseWriter.WriteHeader(code)
}

// Hijack lets nhooyr.io/websocket take over the raw TCP connection.
// Without this, the wrapped responseWriter hides the Hijacker interface
// and websocket.Accept returns 501 Not Implemented.
func (rw *responseWriter) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	return rw.ResponseWriter.(http.Hijacker).Hijack()
}

// logRequest logs an HTTP request with structured fields.
func (s *Server) logRequest(r *http.Request, status int, duration time.Duration) {
	s.logger.LogAttrs(
		r.Context(),
		slog.LevelInfo,
		"http request",
		slog.String("method", r.Method),
		slog.String("path", r.URL.Path),
		slog.Int("status", status),
		slog.Duration("duration", duration),
		slog.String("remote_addr", r.RemoteAddr),
	)
}
