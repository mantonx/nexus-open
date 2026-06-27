package api

import (
	"net/http"
	"runtime"
)

func (s *Server) handleDaemonInfo(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	pluginCount := 0
	if s.pluginCatalog != nil {
		pluginCount = len(s.pluginCatalog.GetCatalog())
	}

	s.respondSuccess(w, "Daemon information", map[string]any{
		"version":      s.buildInfo.Version,
		"commit":       s.buildInfo.Commit,
		"build_time":   s.buildInfo.BuildTime,
		"go_version":   runtime.Version(),
		"plugin_count": pluginCount,
	})
}
