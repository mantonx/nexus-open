export interface UploadingImage {
  id: string;
  file: File;
  previewUrl: string;
  targetName?: string;
  preview: string;
}

export interface ImageGalleryProps {
  images: string[];
  previews: { [key: string]: string };
  onDelete: (image: string) => void;
  uploadingImages: UploadingImage[];
}
