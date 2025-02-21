package configuration

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"image"
	"image/draw"
	"image/gif"
	"image/jpeg"
	_ "image/jpeg" // Register JPEG format
	"image/png"
	_ "image/png" // Register PNG format
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/nfnt/resize"
)

var allowedExtensions = map[string]bool{
	".gif":  true,
	".png":  true,
	".jpg":  true,
	".jpeg": true,
}

const (
	targetWidth  = 640
	targetHeight = 48 // Changed from 480 to match display dimensions
)

// GenerateUniqueFileName creates a unique filename with original extension
func GenerateUniqueFileName(originalName string) string {
	ext := filepath.Ext(originalName)
	name := strings.TrimSuffix(originalName, ext) // Remove extension
	hash := sha256.Sum256([]byte(name))
	return fmt.Sprintf("%x%s", hash[:8], ext)
}

// SaveImage saves and resizes an uploaded image to the images directory
func SaveImage(filename string, data io.Reader) error {
	ext := strings.ToLower(filepath.Ext(filename))
	if !allowedExtensions[ext] {
		return fmt.Errorf("unsupported file type: %s", ext)
	}

	// Ensure images directory exists
	imagesDir, err := GetImagesDir()
	if err != nil {
		return fmt.Errorf("failed to get/create images directory: %w", err)
	}

	// Check if file exists and remove it
	destPath := filepath.Join(imagesDir, filename)
	if _, err := os.Stat(destPath); err == nil {
		if err := os.Remove(destPath); err != nil {
			return fmt.Errorf("failed to remove existing file: %w", err)
		}
	}

	// Read the image data
	imgData, err := io.ReadAll(data)
	if err != nil {
		return fmt.Errorf("failed to read image data: %w", err)
	}

	// Decode the image
	img, format, err := image.Decode(bytes.NewReader(imgData))
	if err != nil {
		return fmt.Errorf("failed to decode image: %w", err)
	}

	// Calculate resize dimensions maintaining aspect ratio
	bounds := img.Bounds()
	ratio := float64(bounds.Dx()) / float64(bounds.Dy())
	newWidth := targetWidth
	newHeight := targetHeight

	if ratio > (float64(targetWidth) / float64(targetHeight)) {
		// Image is wider than target ratio
		newHeight = int(float64(targetWidth) / ratio)
	} else {
		// Image is taller than target ratio
		newWidth = int(float64(targetHeight) * ratio)
	}

	// Resize the image
	resized := resize.Resize(uint(newWidth), uint(newHeight), img, resize.Lanczos3)

	// Create a new RGBA image with the target dimensions
	finalImg := image.NewRGBA(image.Rect(0, 0, targetWidth, targetHeight))

	// Calculate position to center the resized image
	x := (targetWidth - newWidth) / 2
	y := (targetHeight - newHeight) / 2

	// Draw the resized image onto the center of the target image
	draw.Draw(finalImg, finalImg.Bounds(), image.Black, image.Point{}, draw.Src)
	draw.Draw(finalImg, image.Rect(x, y, x+newWidth, y+newHeight), resized, image.Point{}, draw.Over)

	// Create the output file
	out, err := os.Create(destPath)
	if err != nil {
		return err
	}
	defer out.Close()

	// Encode the resized image in the original format
	switch format {
	case "jpeg":
		err = jpeg.Encode(out, finalImg, &jpeg.Options{Quality: 85})
	case "png":
		err = png.Encode(out, finalImg)
	case "gif":
		err = gif.Encode(out, finalImg, &gif.Options{NumColors: 256})
	default:
		return fmt.Errorf("unsupported image format: %s", format)
	}

	if err != nil {
		return err
	}

	return nil
}

// DeleteImage removes an image from the images directory
func DeleteImage(filename string) error {
	imagesDir, err := GetImagesDir()
	if err != nil {
		return fmt.Errorf("failed to get images directory: %w", err)
	}
	return os.Remove(filepath.Join(imagesDir, filename))
}

// ReadImage reads an image file from the images directory
func ReadImage(filename string) ([]byte, error) {
	imagesDir, err := GetImagesDir()
	if err != nil {
		return nil, fmt.Errorf("failed to get images directory: %w", err)
	}

	fullPath := filepath.Join(imagesDir, filename)
	if _, err := os.Stat(fullPath); os.IsNotExist(err) {
		return nil, fmt.Errorf("image file not found: %s", filename)
	}

	data, err := os.ReadFile(fullPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read image file: %w", err)
	}

	return data, nil
}
