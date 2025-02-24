import { BackgroundSettingsProps } from './types';
import { ImageGallery } from '../ImageGallery';

/**
 * BackgroundSettings Component
 *
 * Handles background image uploads and management.
 */
export function BackgroundSettings({
  label,
  images,
  previews,
  onImageUpload,
  onImageDelete,
  uploadingImages
}: BackgroundSettingsProps) {
  return (
    <div className="settings-section">
      <h2>{label}</h2>
      <div className="form-group">
        <label htmlFor="images">Upload Images</label>
        <input
          type="file"
          id="images"
          multiple
          accept=".gif,.png,.jpg,.jpeg"
          onChange={onImageUpload}
        />
      </div>
      {images.length > 0 && (
        <div className="image-list">
          <h3>Uploaded Images (Max 10)</h3>
          <ImageGallery
            images={images}
            previews={previews}
            onDelete={onImageDelete}
            uploadingImages={uploadingImages}
          />
        </div>
      )}
    </div>
  );
}
