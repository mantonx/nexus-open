package instruments

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"
	"time"
)

type NewsItem struct {
	Title       string    `json:"title"`
	Description string    `json:"description"`
	PublishedAt time.Time `json:"publishedAt"`
}

func GetLatestNews() (*NewsItem, error) {
	// Replace with your actual API key and endpoint
	apiKey := "your-api-key"
	url := "https://newsapi.org/v2/top-headlines?country=us&apiKey=" + apiKey

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(url)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch news: %v", err)
	}
	defer resp.Body.Close()

	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %v", err)
	}

	var result struct {
		Articles []NewsItem `json:"articles"`
	}

	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to parse JSON: %v", err)
	}

	if len(result.Articles) == 0 {
		return nil, fmt.Errorf("no news articles found")
	}

	// Truncate the title if it's longer than 50 characters
	news := result.Articles[0]
	if len(news.Title) > 50 {
		news.Title = news.Title[:47] + "..."
	}

	return &news, nil
}
