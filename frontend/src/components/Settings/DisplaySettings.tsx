import { TabPanelProps } from './types';

interface DisplaySettingsProps extends TabPanelProps {
  timeFormat: string;
  unit: string;
  onTimeFormatChange: (format: string) => void;
  onUnitChange: (unit: string) => void;
}

/**
 * DisplaySettings Component
 *
 * Handles time format and measurement unit preferences.
 */
export function DisplaySettings({ timeFormat, unit, onTimeFormatChange, onUnitChange }: DisplaySettingsProps) {
  return (
    <div className="settings-section">
      <h2>Display Preferences</h2>
      <div className="form-group">
        <label htmlFor="timeFormat">Time Format</label>
        <select
          id="timeFormat"
          value={timeFormat}
          onChange={(e) => onTimeFormatChange(e.target.value)}
          style={{ cursor: 'pointer' }}
        >
          <option value="12h">12 Hour</option>
          <option value="24h">24 Hour</option>
        </select>
      </div>

      <div className="form-group">
        <label htmlFor="unit">Units</label>
        <select
          id="unit"
          value={unit}
          onChange={(e) => onUnitChange(e.target.value)}
          style={{ cursor: 'pointer' }}
        >
          <option value="metric">Metric (°C, km/h)</option>
          <option value="imperial">Imperial (°F, mph)</option>
        </select>
      </div>
    </div>
  );
}
