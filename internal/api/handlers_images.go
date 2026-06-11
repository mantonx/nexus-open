package api

import (
	"net/http"
	"path/filepath"

	"github.com/mantonx/nexus-open/internal/assets"
)

// handleImageUpload processes image uploads.
func (s *Server) handleImageUpload(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Parse multipart form (max 10MB)
	if err := r.ParseMultipartForm(10 << 20); err != nil {
		s.respondError(w, "Failed to parse form: "+err.Error(), http.StatusBadRequest)
		return
	}

	file, header, err := r.FormFile("image")
	if err != nil {
		s.respondError(w, "Failed to read image file: "+err.Error(), http.StatusBadRequest)
		return
	}
	defer func() { _ = file.Close() }()

	// Save image
	if err := assets.SaveImage(header.Filename, file); err != nil {
		s.logger.Error("failed to save image", "error", err, "filename", header.Filename)
		s.respondError(w, "Failed to save image", http.StatusInternalServerError)
		return
	}

	s.logger.Info("image uploaded", "filename", header.Filename, "size", header.Size)
	s.respondSuccess(w, "Image uploaded successfully", map[string]string{
		"filename": header.Filename,
	})
}

// handleListImages returns a list of all uploaded images.
func (s *Server) handleListImages(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	images, err := assets.GetImages()
	if err != nil {
		s.logger.Error("failed to list images", "error", err)
		s.respondError(w, "Failed to list images", http.StatusInternalServerError)
		return
	}

	s.respondJSON(w, images, http.StatusOK)
}

// handleServeImage serves a single image file by filename.
func (s *Server) handleServeImage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Path is /api/images/<filename>
	filename := filepath.Base(r.PathValue("filename"))
	if filename == "" || filename == "." {
		s.respondError(w, "Filename required", http.StatusBadRequest)
		return
	}

	imagesDir, err := assets.GetImagesDir()
	if err != nil {
		s.respondError(w, "Images directory unavailable", http.StatusInternalServerError)
		return
	}

	http.ServeFile(w, r, filepath.Join(imagesDir, filename))
}

// handleDeleteImage deletes an uploaded image.
func (s *Server) handleDeleteImage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost && r.Method != http.MethodDelete {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	filename := filepath.Base(r.FormValue("filename"))
	if filename == "" || filename == "." {
		s.respondError(w, "Missing filename parameter", http.StatusBadRequest)
		return
	}

	if err := assets.DeleteImage(filename); err != nil {
		s.logger.Error("failed to delete image", "error", err, "filename", filename)
		s.respondError(w, "Failed to delete image", http.StatusInternalServerError)
		return
	}

	s.logger.Info("image deleted", "filename", filename)
	s.respondSuccess(w, "Image deleted successfully", nil)
}
