import { SettingsProps } from './types';

/**
 * LocationSettings Component
 *
 * Handles the location input for weather data.
 */
export function LocationSettings({ value, onChange, label }: SettingsProps & { label: string }) {
  return (
    <div className="settings-section">
      <h2>{label}</h2>
      <div className="form-group">
        <label htmlFor="location">Location</label>
        <input
          type="text"
          id="location"
          name="location"
          placeholder="Enter your city"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          autoComplete="off"
          spellCheck="false"
        />
      </div>
    </div>
  );
}
