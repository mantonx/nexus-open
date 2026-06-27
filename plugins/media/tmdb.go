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

// tmdbToken is injected at release build time via:
//
//	go build -ldflags "-X main.tmdbToken=<token>"
//
// In dev builds this is empty and the file fallback is used instead.
var tmdbToken string

// tmdbCache caches poster URLs by title to avoid hitting the API on every sample.
type tmdbCache struct {
	mu      sync.Mutex
	entries map[string]tmdbEntry
}

type tmdbEntry struct {
	posterURL string
	fetchedAt time.Time
}

const tmdbCacheTTL = 24 * time.Hour

var tmdb = &tmdbCache{entries: make(map[string]tmdbEntry)}

// posterURL returns the TMDb poster URL for the given title, or "" if not found.
// Results are cached for 24 hours. The API token is read from ~/.config/nexus-open/tmdb-token.
func (c *tmdbCache) posterURL(title string) string {
	if title == "" {
		return ""
	}

	c.mu.Lock()
	if e, ok := c.entries[title]; ok && time.Since(e.fetchedAt) < tmdbCacheTTL {
		c.mu.Unlock()
		return e.posterURL
	}
	c.mu.Unlock()

	token := readTMDbToken()
	if token == "" {
		return ""
	}

	poster := fetchTMDbPoster(title, token)

	c.mu.Lock()
	c.entries[title] = tmdbEntry{posterURL: poster, fetchedAt: time.Now()}
	c.mu.Unlock()

	return poster
}

func readTMDbToken() string {
	if tmdbToken != "" {
		return tmdbToken
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

func fetchTMDbPoster(title, token string) string {
	req, err := http.NewRequest("GET", tmdbSearchURL, nil)
	if err != nil {
		return ""
	}
	q := url.Values{}
	q.Set("query", title)
	q.Set("language", "en-US")
	q.Set("page", "1")
	req.URL.RawQuery = q.Encode()
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", token))

	client := &http.Client{Timeout: 4 * time.Second}
	resp, err := client.Do(req)
	if err != nil || resp.StatusCode != 200 {
		return ""
	}
	defer resp.Body.Close()

	var result struct {
		Results []struct {
			PosterPath string `json:"poster_path"`
		} `json:"results"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return ""
	}
	if len(result.Results) == 0 || result.Results[0].PosterPath == "" {
		return ""
	}
	return tmdbImageBase + result.Results[0].PosterPath
}
