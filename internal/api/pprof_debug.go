//go:build pprof

package api

import (
	"net/http"
	_ "net/http/pprof"
)

func init() {
	go func() {
		http.ListenAndServe("127.0.0.1:1986", nil) //nolint:errcheck
	}()
}
