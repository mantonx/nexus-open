import { useState } from 'react';
import { ImageGalleryProps } from './types';

/**
 * ImageGallery Component
 *
 * Displays a paginated grid of images with upload states and delete functionality.
 * Supports both uploaded images and images in the process of being uploaded.
 *
 * @param props.images - Array of image paths
 * @param props.previews - Object mapping image paths to preview URLs
 * @param props.onDelete - Callback function when an image is deleted
 * @param props.uploadingImages - Array of images currently being uploaded
 */
export function ImageGallery({ images, previews, onDelete, uploadingImages }: ImageGalleryProps) {
  const [startIndex, setStartIndex] = useState(0);
  const [errorImages, setErrorImages] = useState<Set<string>>(new Set());
  const imagesPerPage = 4;

  // Combine regular images and uploading images
  const displayImages = [
    ...images
      .filter(path => !errorImages.has(path))
      .map(path => ({ path, preview: previews[path], isUploading: false })),
    ...uploadingImages.map(img => ({ path: img.id, preview: img.previewUrl, isUploading: true }))
  ];

  const handleImageError = (path: string) => {
    console.log('Image failed to load:', path);
    setErrorImages(prev => new Set([...prev, path]));
    onDelete(path); // Remove the broken image from config
  };

  const totalPages = Math.ceil(displayImages.length / imagesPerPage);
  const currentPage = Math.floor(startIndex / imagesPerPage) + 1;
  const currentImages = displayImages.slice(startIndex, startIndex + imagesPerPage);

  const showNextButton = startIndex + imagesPerPage < displayImages.length;
  const showPrevButton = startIndex > 0;

  return (
    <div className="image-gallery">
      {showPrevButton && (
        <button className="gallery-nav prev" onClick={() => setStartIndex(prev => prev - imagesPerPage)}>
          &lt;
        </button>
      )}

      <div className="gallery-grid">
        {currentImages.map(({ path, preview, isUploading }) => (
          <div key={path} className="gallery-item">
            <div className="image-container">
              <img
                src={preview || '/placeholder.png'} // Add a placeholder image
                alt={path}
                onError={() => !isUploading && handleImageError(path)}
                onLoad={(e) => {
                  const img = e.target as HTMLImageElement;
                  img.style.opacity = '1';
                }}
                style={{ opacity: 0, transition: 'opacity 0.3s ease-in-out' }}
              />
              {isUploading ? (
                <div className="image-loader">
                  <div className="spinner"></div>
                  <span>Uploading...</span>
                </div>
              ) : (
                <button
                  className="delete-button"
                  onClick={() => onDelete(path)}
                  title="Remove image"
                >
                  ×
                </button>
              )}
            </div>
            <div className="gallery-controls">
              <span title={path}>{path}</span>
            </div>
          </div>
        ))}
      </div>

      {showNextButton && (
        <button className="gallery-nav next" onClick={() => setStartIndex(prev => prev + imagesPerPage)}>
          &gt;
        </button>
      )}

      {totalPages > 1 && (
        <div className="gallery-pagination">
          Page {currentPage} of {totalPages}
        </div>
      )}
    </div>
  );
}
