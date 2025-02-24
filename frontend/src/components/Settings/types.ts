import { ReactNode } from 'react';

export interface TabPanelProps {
  label: string;
}

export interface TabContentProps extends TabPanelProps {
  children?: ReactNode;
}

export interface SettingsProps extends TabPanelProps {
  value: string;
  onChange: (value: string) => void;
}

export interface DisplaySettingsProps extends TabPanelProps {
  timeFormat: string;
  unit: string;
  onTimeFormatChange: (format: string) => void;
  onUnitChange: (unit: string) => void;
}

export interface AppearanceSettingsProps extends TabPanelProps {
  backgroundColor: string;
  textColor: string;
  onBackgroundColorChange: (color: string) => void;
  onTextColorChange: (color: string) => void;
}

export interface BackgroundSettingsProps extends TabPanelProps {
  images: string[];
  previews: { [key: string]: string };
  onImageUpload: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onImageDelete: (filename: string) => void;
  uploadingImages: any[];
}

export interface TabProps {
  label: string;
  isActive: boolean;
  onClick: () => void;
}

export interface TabsProps {
  children: React.ReactElement<TabPanelProps>[];
}
