package nexus

import (
	"encoding/json"
	"net/http"

	"nexus-open/nexus/configuration"
)

// SetupAPI registers HTTP endpoints for:
//  1. reading/updating configuration   (/api/config)
//  2. uploading images                 (/api/images/upload)
//  3. listing images                   (/api/images)
//  4. deleting images                  (/api/images/delete)
func SetupAPI() {
	// Single config endpoint handles both GET (read) and POST (update)
	http.HandleFunc("/api/config", configHandler)
	http.HandleFunc("/api/images/upload", uploadImageHandler)
	http.HandleFunc("/api/images", listImagesHandler)
	http.HandleFunc("/api/images/delete", deleteImageHandler)
	http.ListenAndServe(":1985", nil)
}

// configHandler handles reading (GET) and updating (POST) configuration.
func configHandler(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		config, err := configuration.LoadConfig("")
		if err != nil {
			http.Error(w, "Failed to read config", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(config)
	case http.MethodPost:
		var newConfig configuration.NexusConfig
		if err := json.NewDecoder(r.Body).Decode(&newConfig); err != nil {
			http.Error(w, "Bad request", http.StatusBadRequest)
			return
		}
		if err := configuration.SaveConfig(&newConfig, ""); err != nil {
			http.Error(w, "Failed to save config", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.Write([]byte(`{"status":"ok"}`))
	default:
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// uploadImageHandler processes image uploads via multipart form data.
func uploadImageHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	file, header, err := r.FormFile("image")

	if err != nil {
		http.Error(w, "Failed to read file form field", http.StatusBadRequest)
		return
	}
	defer file.Close()

	err = configuration.SaveImage(header.Filename, file)
	if err != nil {
		http.Error(w, "Failed to save image", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(`{"status":"ok"}`))
}

// listImagesHandler returns a list of available images (GET).
func listImagesHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	images, err := configuration.GetImages()
	if err != nil {
		http.Error(w, "Failed to read images", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(images)
}

// deleteImageHandler removes an image from the server (POST).
func deleteImageHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	filename := r.FormValue("filename")
	if filename == "" {
		http.Error(w, "Missing filename", http.StatusBadRequest)
		return
	}

	err := configuration.DeleteImage(filename)
	if err != nil {
		http.Error(w, "Failed to delete image", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(`{"status":"ok"}`))
}
