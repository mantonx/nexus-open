package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"
)

const (
	tmdbSearchURL = "https://api.themoviedb.org/3/search/multi"
	tmdbImageBase = "https://image.tmdb.org/t/p/w92"
	tmdbTokenFile = ".config/nexus-open/tmdb-token"
)

// bundledTMDbTokenHex holds the bundled API token XORed against the key "NXOR"
// and encoded as a hex string. This is a speed bump — the token is recoverable
// from any build, but it won't appear as a recognisable string under `strings`.
// Users can supply their own token at ~/.config/nexus-open/tmdb-token instead.
//
// To inject at release build time, encode the token first:
//
//	scripts/xor-token.sh <token>
//
// Then pass the output to:
//
//	go build -ldflags "-X main.bundledTMDbTokenHex=<hex>"
var bundledTMDbTokenHex string

func bundledTMDbToken() string {
	if bundledTMDbTokenHex == "" {
		return ""
	}
	key := []byte("NXOR")
	src := bundledTMDbTokenHex
	out := make([]byte, len(src)/2)
	for i := range out {
		var b byte
		fmt.Sscanf(src[i*2:i*2+2], "%02x", &b)
		out[i] = b ^ key[i%len(key)]
	}
	return string(out)
}

// tmdbEntry is a cache record. found=false means a confirmed "not found" result
// (negative cache). Both positive and negative entries expire independently.
type tmdbEntry struct {
	posterURL string
	found     bool
	fetchedAt time.Time
}

const (
	tmdbCacheTTL         = 24 * time.Hour
	tmdbNegativeCacheTTL = 6 * time.Hour  // retry "not found" after 6h
	tmdbBackoffMax       = 30 * time.Minute
)

type tmdbCache struct {
	mu          sync.Mutex
	entries     map[string]tmdbEntry
	backoffUntil time.Time // don't attempt any request until this time
	backoff      time.Duration
}

var tmdb = &tmdbCache{entries: make(map[string]tmdbEntry)}

// posterURL returns the TMDb poster URL for the given title, or "" if not found
// or on any error. Results (including "not found") are cached to avoid hammering
// the API. Transient errors trigger exponential backoff up to 30 minutes.
func (c *tmdbCache) posterURL(title string) string {
	if title == "" {
		return ""
	}

	c.mu.Lock()
	if e, ok := c.entries[title]; ok {
		ttl := tmdbCacheTTL
		if !e.found {
			ttl = tmdbNegativeCacheTTL
		}
		if time.Since(e.fetchedAt) < ttl {
			c.mu.Unlock()
			return e.posterURL
		}
	}
	if time.Now().Before(c.backoffUntil) {
		c.mu.Unlock()
		return ""
	}
	c.mu.Unlock()

	token := readTMDbToken()
	if token == "" {
		return ""
	}

	poster, ok := fetchTMDbPoster(title, token)

	c.mu.Lock()
	if !ok {
		// Transient error — back off exponentially, don't cache.
		if c.backoff == 0 {
			c.backoff = 30 * time.Second
		} else {
			c.backoff *= 2
			if c.backoff > tmdbBackoffMax {
				c.backoff = tmdbBackoffMax
			}
		}
		c.backoffUntil = time.Now().Add(c.backoff)
	} else {
		// Success (poster may be "" for "not found") — reset backoff, cache result.
		c.backoff = 0
		c.entries[title] = tmdbEntry{
			posterURL: poster,
			found:     poster != "",
			fetchedAt: time.Now(),
		}
	}
	c.mu.Unlock()

	return poster
}

func readTMDbToken() string {
	if t := bundledTMDbToken(); t != "" {
		return t
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	data, err := os.ReadFile(home + "/" + tmdbTokenFile)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

// fetchTMDbPoster fetches the poster URL for title. Returns ("", true) when the
// API responds but no match is found. Returns ("", false) on any transient error
// (network failure, non-200 status) so the caller can apply backoff.
func fetchTMDbPoster(title, token string) (string, bool) {
	req, err := http.NewRequest("GET", tmdbSearchURL, nil)
	if err != nil {
		return "", false
	}
	q := url.Values{}
	q.Set("query", title)
	q.Set("language", "en-US")
	q.Set("page", "1")
	req.URL.RawQuery = q.Encode()
	req.Header.Set("Authorization", "Bearer "+token)

	client := &http.Client{Timeout: 4 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return "", false
	}
	defer resp.Body.Close()
	if resp.StatusCode == http.StatusTooManyRequests {
		return "", false // treat rate-limit as transient
	}
	if resp.StatusCode != http.StatusOK {
		return "", false
	}

	var result struct {
		Results []struct {
			PosterPath string `json:"poster_path"`
		} `json:"results"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", false
	}
	if len(result.Results) == 0 || result.Results[0].PosterPath == "" {
		return "", true // definitive "not found" — cache as negative
	}
	return tmdbImageBase + result.Results[0].PosterPath, true
}
