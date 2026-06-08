package api

import (
	"bufio"
	"crypto/subtle"
	"net"
	"net/http"
	"strings"
	"time"
)

// middleware applies all middleware to the handler chain.
func (s *Server) middleware(next http.Handler) http.Handler {
	return s.loggingMiddleware(s.localOnlyMiddleware(next))
}

// localOnlyMiddleware rejects requests that don't come from a local client.
// It enforces two independent checks:
//
//  1. Host header must be localhost or 127.0.0.1 (with the server port).
//     This closes DNS-rebinding: a browser page at attacker.com can resolve
//     to 127.0.0.1, but the request still carries "Host: attacker.com", which
//     we reject here.
//
//  2. X-Nexus-Token must match the per-user token stored at
//     ~/.config/nexus-open/token (mode 0600).  This protects multi-user
//     machines and any non-bundled local process.
func (s *Server) localOnlyMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// --- Host validation ---
		host := r.Host
		// Strip port suffix for the pure-hostname comparison.
		if h, _, err := net.SplitHostPort(host); err == nil {
			host = h
		}
		switch host {
		case "localhost", "127.0.0.1", "::1":
			// allowed
		default:
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}

		// --- Token validation ---
		// Exempt /api/health so waitForFlutter can probe before the client
		// has loaded the token file.
		// WebSocket upgrades send the token as ?token= because the WS handshake
		// HTTP request cannot carry arbitrary headers from most WS clients.
		if r.URL.Path != "/api/health" {
			var tok string
			if r.URL.Path == "/api/ws" {
				tok = strings.TrimSpace(r.URL.Query().Get("token"))
			} else {
				tok = strings.TrimSpace(r.Header.Get("X-Nexus-Token"))
			}
			if s.token == "" || tok == "" || subtle.ConstantTimeCompare([]byte(tok), []byte(s.token)) != 1 {
				http.Error(w, "unauthorized", http.StatusUnauthorized)
				return
			}
		}

		next.ServeHTTP(w, r)
	})
}

// loggingMiddleware logs all HTTP requests.
func (s *Server) loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()

		wrapped := &responseWriter{
			ResponseWriter: w,
			statusCode:     http.StatusOK,
		}

		next.ServeHTTP(wrapped, r)

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

// Hijack lets the websocket library take over the raw TCP connection.
// Without this, the wrapped responseWriter hides the Hijacker interface
// and websocket.Accept returns 501 Not Implemented.
func (rw *responseWriter) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	return rw.ResponseWriter.(http.Hijacker).Hijack()
}
