import './App.css'
import { useState, useEffect } from 'react'
import { UpdateConfig, GetConfig, GetImagePreview, UploadImage, DeleteImage } from '../wailsjs/go/main/App'
import { main } from '../wailsjs/go/models'

interface UploadingImage {
  id: string;
  file: File;
  previewUrl: string;
  targetName?: string; // Add this to track the final filename
}

function ImageGallery({ images, previews, onDelete, uploadingImages }: {
  images: string[];
  previews: { [key: string]: string };
  onDelete: (image: string) => void;
  uploadingImages: UploadingImage[];
}) {
  const [startIndex, setStartIndex] = useState(0);
  const imagesPerPage = 4;
  const maxImages = 10;

  // Keep track of all images in order, including those being uploaded
  const displayImages = [...uploadingImages.map(img => img.id), ...images]
    .filter(path => uploadingImages.some(img => img.id === path) || previews[path])
    .slice(0, maxImages);

  // Create combined previews object
  const allPreviews = {
    ...previews,
    ...Object.fromEntries(uploadingImages.map(img => [img.id, img.previewUrl]))
  };

  const totalPages = Math.ceil(displayImages.length / imagesPerPage);

  useEffect(() => {
    // Reset to first page when images change
    setStartIndex(0);
  }, [displayImages.length]);

  const nextPage = () => {
    const nextIndex = startIndex + imagesPerPage;
    if (nextIndex < displayImages.length) {
      setStartIndex(nextIndex);
    }
  };

  const prevPage = () => {
    const prevIndex = startIndex - imagesPerPage;
    if (prevIndex >= 0) {
      setStartIndex(prevIndex);
    }
  };

  if (displayImages.length === 0) return null;

  const currentImages = displayImages.slice(startIndex, startIndex + imagesPerPage);
  const pageNumber = Math.floor(startIndex / imagesPerPage) + 1;

  // Fix: Only show next button when there are more images than can fit on the current page
  const showNextButton = displayImages.length > (startIndex + imagesPerPage);
  const showPrevButton = startIndex > 0;

  return (
    <div className="image-gallery">
      {showPrevButton && (
        <button
          className="gallery-nav prev"
          onClick={prevPage}
        >
          &lt;
        </button>
      )}
      <div className="gallery-grid">
        {Array(imagesPerPage).fill(null).map((_, index) => {
          const path = currentImages[index] || '';
          const isUploading = uploadingImages.some(img => img.id === path);
          const preview = allPreviews[path];

          return (
            <div key={path || `empty-${index}`} className="gallery-item">
              {path && preview && (
                <>
                  <div className="image-container">
                    <img
                      src={preview}
                      alt={path}
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
                </>
              )}
            </div>
          );
        })}
      </div>
      {showNextButton && (
        <button
          className="gallery-nav next"
          onClick={nextPage}
          disabled={startIndex + imagesPerPage >= displayImages.length}
        >
          &gt;
        </button>
      )}
      {(showPrevButton || showNextButton) && (
        <div className="gallery-pagination">
          Page {pageNumber} of {totalPages}
        </div>
      )}
    </div>
  );
}

function App() {
  const [config, setConfig] = useState({
    location: '',
    time_format: '24h',
    unit: 'celsius',
    background_color: '#FFFFFF',
    text_color: '#000000',
    image_paths: [] as string[]
  })
  const [previews, setPreviews] = useState<{ [key: string]: string }>({});
  const [uploadingImages, setUploadingImages] = useState<UploadingImage[]>([]);

  useEffect(() => {
    const loadConfig = async () => {
      try {
        const savedConfig = await GetConfig()
        setConfig(savedConfig)

        // Immediately load previews for any existing images
        if (savedConfig.image_paths && savedConfig.image_paths.length > 0) {
          console.log('Loading previews for:', savedConfig.image_paths);
          await loadImagePreviews(savedConfig.image_paths);
        }
      } catch (error) {
        console.error('Error loading config:', error)
      }
    }
    loadConfig()
  }, [])

  const handleSubmit = async () => {
    try {
      await UpdateConfig(config)
    } catch (error) {
      console.error('Error updating config:', error)
    }
  }

  const handleImageUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files) return;

    const validFiles = Array.from(files).filter(file => {
      const type = file.type.toLowerCase();
      return type === 'image/gif' || type === 'image/png' || type === 'image/jpeg';
    });

    // Create uploading images at the start of the list
    const newUploadingImages = validFiles.map(file => ({
      id: `uploading-${Date.now()}-${file.name}`,
      file,
      previewUrl: URL.createObjectURL(file)
    }));

    // Add new uploading images to the start of the list
    setUploadingImages(prev => [...newUploadingImages, ...prev]);

    for (const uploadingImage of newUploadingImages) {
      try {
        const arrayBuffer = await uploadingImage.file.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);
        const imageInfo = await UploadImage(uploadingImage.file.name, Array.from(bytes));

        // Load the final preview
        const base64Data = await GetImagePreview(imageInfo.storedName);

        // Update config first
        setConfig(prev => ({
          ...prev,
          image_paths: [imageInfo.storedName, ...prev.image_paths]
        }));

        // Then update previews
        setPreviews(prev => ({
          ...prev,
          [imageInfo.storedName]: `data:image;base64,${base64Data}`
        }));

        // Remove uploading state after short delay
        await new Promise(resolve => setTimeout(resolve, 300));
        setUploadingImages(prev =>
          prev.filter(img => img.id !== uploadingImage.id)
        );

        // Clean up
        URL.revokeObjectURL(uploadingImage.previewUrl);

      } catch (error) {
        console.error('Error uploading file:', uploadingImage.file.name, error);
        setUploadingImages(prev =>
          prev.filter(img => img.id !== uploadingImage.id)
        );
        URL.revokeObjectURL(uploadingImage.previewUrl);
      }
    }

    event.target.value = '';
  };

  const handleImageDelete = async (filename: string) => {
    try {
      await DeleteImage(filename);
      setConfig(prev => ({
        ...prev,
        image_paths: prev.image_paths.filter(path => path !== filename)
      }));
      setPreviews(prev => {
        const newPreviews = { ...prev };
        delete newPreviews[filename];
        return newPreviews;
      });
    } catch (error) {
      console.error('Error deleting image:', error);
    }
  };

  const loadImagePreviews = async (paths: string[]) => {
    const newPreviews: { [key: string]: string } = {};
    for (const path of paths) {
      try {
        console.log('Loading preview for:', path);
        const base64Data = await GetImagePreview(path);
        if (base64Data) {
          newPreviews[path] = `data:image;base64,${base64Data}`;
          console.log('Preview loaded for:', path);
        }
      } catch (error) {
        console.error('Error loading preview for', path, error);
      }
    }
    setPreviews(prev => ({ ...prev, ...newPreviews }));
  };

  const handleLocationChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setConfig(prev => ({
      ...prev,
      location: value
    }));
  };

  return (
    <div className="app">
      <div className="settings-section">
        <h2>Location Settings</h2>
        <div className="form-group">
          <label htmlFor="location">Location</label>
          <input
            type="text"
            id="location"
            name="location"
            placeholder="Enter your city"
            value={config.location}
            onChange={handleLocationChange}
            autoComplete="off"
            spellCheck="false"
          />
        </div>
      </div>

      <div className="settings-section">
        <h2>Display Preferences</h2>
        <div className="form-group">
          <label htmlFor="timeFormat">Time Format</label>
          <select
            id="timeFormat"
            value={config.time_format}
            onChange={(e) => setConfig({ ...config, time_format: e.target.value })}
          >
            <option value="12h">12 Hour</option>
            <option value="24h">24 Hour</option>
          </select>
        </div>

        <div className="form-group">
          <label htmlFor="unit">Units</label>
          <select
            id="unit"
            value={config.unit}
            onChange={(e) => setConfig({ ...config, unit: e.target.value })}
          >
            <option value="metric">Metric (°C, km/h)</option>
            <option value="imperial">Imperial (°F, mph)</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <h2>Appearance</h2>
        <div className="form-group">
          <label htmlFor="backgroundColor">Background Color</label>
          <input
            type="color"
            id="backgroundColor"
            value={config.background_color}
            onChange={(e) => setConfig({ ...config, background_color: e.target.value })}
          />
          <div
            className="color-preview"
            style={{ backgroundColor: config.background_color }}
          />
        </div>

        <div className="form-group">
          <label htmlFor="textColor">Text Color</label>
          <input
            type="color"
            id="textColor"
            value={config.text_color}
            onChange={(e) => setConfig({ ...config, text_color: e.target.value })}
          />
          <div
            className="color-preview"
            style={{ backgroundColor: config.text_color }}
          />
        </div>
      </div>

      <div className="settings-section">
        <h2>Background Images</h2>
        <div className="form-group">
          <label htmlFor="images">Upload Images</label>
          <input
            type="file"
            id="images"
            multiple
            accept=".gif,.png,.jpg,.jpeg"
            onChange={handleImageUpload}
          />
        </div>
        {config.image_paths.length > 0 && (
          <div className="image-list">
            <h3>Uploaded Images (Max 10)</h3>
            <ImageGallery
              images={config.image_paths}
              previews={previews}
              onDelete={handleImageDelete}
              uploadingImages={uploadingImages}
            />
          </div>
        )}
      </div>

      <button className="save-button" onClick={handleSubmit}>
        Save Changes
      </button>
    </div >
  )
}

export default App
