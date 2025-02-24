import './App.css'
import { useState, useEffect } from 'react'
import { UpdateConfig, GetConfig, GetImagePreview, UploadImage, DeleteImage } from '../wailsjs/go/main/App'
import { LocationSettings } from './components/Settings/LocationSettings'
import { DisplaySettings } from './components/Settings/DisplaySettings'
import { AppearanceSettings } from './components/Settings/AppearanceSettings'
import { BackgroundSettings } from './components/Settings/BackgroundSettings'
import { UploadingImage } from './components/ImageGallery/types'
import { Tabs } from './components/Settings/Tabs';

function App() {
  const [config, setConfig] = useState({
    location: '',
    time_format: '24h',
    unit: 'celsius',
    background_color: '#FFFFFF',
    text_color: '#FFFFFF',
    image_paths: [] as string[]
  })
  const [previews, setPreviews] = useState<{ [key: string]: string }>({});
  const [uploadingImages, setUploadingImages] = useState<UploadingImage[]>([]);

  useEffect(() => {
    const
      loadConfig = async () => {
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
      previewUrl: URL.createObjectURL(file),
      preview: URL.createObjectURL(file)
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
    const failedImages: string[] = [];

    for (const path of paths) {
      try {
        const base64Data = await GetImagePreview(path);
        if (base64Data) {
          newPreviews[path] = `data:image;base64,${base64Data}`;
        } else {
          failedImages.push(path);
        }
      } catch (error) {
        console.error('Error loading preview for', path, error);
        failedImages.push(path);
      }
    }

    // Remove failed images from config
    if (failedImages.length > 0) {
      setConfig(prev => ({
        ...prev,
        image_paths: prev.image_paths.filter(path => !failedImages.includes(path))
      }));
    }

    setPreviews(prev => ({ ...prev, ...newPreviews }));
  };

  return (
    <div className="app">
      <Tabs>
        <LocationSettings
          label="Location"
          value={config.location}
          onChange={(value) => setConfig(prev => ({ ...prev, location: value }))}
        />

        <DisplaySettings
          label="Display"
          timeFormat={config.time_format}
          unit={config.unit}
          onTimeFormatChange={(format) => setConfig(prev => ({ ...prev, time_format: format }))}
          onUnitChange={(unit) => setConfig(prev => ({ ...prev, unit }))}
        />

        <AppearanceSettings
          label="Appearance"
          backgroundColor={config.background_color}
          textColor={config.text_color}
          onBackgroundColorChange={(color) => setConfig(prev => ({ ...prev, background_color: color }))}
          onTextColorChange={(color) => setConfig(prev => ({ ...prev, text_color: color }))}
        />

        <BackgroundSettings
          label="Background Images"
          images={config.image_paths}
          previews={previews}
          onImageUpload={handleImageUpload}
          onImageDelete={handleImageDelete}
          uploadingImages={uploadingImages}
        />
      </Tabs>

      <button className="save-button" onClick={handleSubmit}>
        Save Changes
      </button>
    </div>
  );
}

export default App
