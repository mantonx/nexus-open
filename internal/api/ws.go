package api

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"sync"

	"nhooyr.io/websocket"
	"nhooyr.io/websocket/wsjson"
)

// WSMessage is a typed envelope for all WebSocket messages sent to clients.
type WSMessage struct {
	Type string `json:"type"` // "frame" | "window_state" | "config"
	Data any    `json:"data"`
}

// hub manages all connected WebSocket clients and broadcasts messages to them.
type hub struct {
	mu      sync.Mutex
	clients map[chan WSMessage]struct{}
	logger  *slog.Logger
}

func newHub(logger *slog.Logger) *hub {
	return &hub{
		clients: make(map[chan WSMessage]struct{}),
		logger:  logger,
	}
}

// subscribe returns a channel that will receive broadcast messages.
// The caller must call unsubscribe when done to avoid a goroutine leak.
func (h *hub) subscribe() chan WSMessage {
	ch := make(chan WSMessage, 16)
	h.mu.Lock()
	h.clients[ch] = struct{}{}
	h.mu.Unlock()
	return ch
}

func (h *hub) unsubscribe(ch chan WSMessage) {
	h.mu.Lock()
	delete(h.clients, ch)
	h.mu.Unlock()
}

// Broadcast sends msg to all connected clients, dropping slow ones.
func (h *hub) Broadcast(msg WSMessage) {
	h.mu.Lock()
	defer h.mu.Unlock()
	for ch := range h.clients {
		select {
		case ch <- msg:
		default:
			// Client is too slow — drop the message rather than blocking the render loop.
		}
	}
}

// handleWS upgrades the connection and streams hub messages to the client.
func (s *Server) handleWS(w http.ResponseWriter, r *http.Request) {
	conn, err := websocket.Accept(w, r, &websocket.AcceptOptions{
		// Only allow connections originating from localhost — prevents a
		// malicious remote page from hijacking the WebSocket via CSWSH.
		OriginPatterns: []string{"localhost:*", "127.0.0.1:*"},
	})
	if err != nil {
		s.logger.Debug("WebSocket upgrade failed", "error", err)
		return
	}
	defer conn.CloseNow()

	ch := s.hub.subscribe()
	defer s.hub.unsubscribe(ch)

	ctx := conn.CloseRead(r.Context())

	// Send current window state immediately on connect so the client is in sync.
	s.hub.mu.Lock()
	_ = wsjson.Write(ctx, conn, WSMessage{Type: "window_state", Data: s.windowState})
	s.hub.mu.Unlock()

	for {
		select {
		case <-ctx.Done():
			return
		case msg, ok := <-ch:
			if !ok {
				return
			}
			if err := wsjson.Write(ctx, conn, msg); err != nil {
				s.logger.Debug("WebSocket write error, dropping client", "error", err)
				return
			}
		}
	}
}

// broadcastWindowState sends a window_state message to all hub clients.
func (s *Server) broadcastWindowState(state string) {
	data, _ := json.Marshal(state)
	_ = data
	s.hub.Broadcast(WSMessage{Type: "window_state", Data: state})
}
