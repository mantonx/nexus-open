import { AppearanceSettingsProps } from './types';
import { ColorPicker } from '../common/ColorPicker';

/**
 * AppearanceSettings Component
 *
 * Handles background and text color selection with live previews.
 */
export function AppearanceSettings({
  label,
  backgroundColor,
  textColor,
  onBackgroundColorChange,
  onTextColorChange
}: AppearanceSettingsProps) {
  return (
    <div className="settings-section">
      <h2>{label}</h2>
      <div className="form-group">
        <ColorPicker
          label="Background Color"
          color={backgroundColor}
          onChange={onBackgroundColorChange}
        />
      </div>
      <div className="form-group">
        <ColorPicker
          label="Text Color"
          color={textColor}
          onChange={onTextColorChange}
        />
      </div>
    </div>
  );
}
