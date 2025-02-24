import { useState } from 'react';
import './styles.css';

interface ColorPickerProps {
  label: string;
  color: string;
  onChange: (color: string) => void;
}

export function ColorPicker({ label, color, onChange }: ColorPickerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isValid, setIsValid] = useState(true);

  const handleColorChange = (value: string) => {
    const isValidColor = /^#[0-9A-Fa-f]{6}$/.test(value);
    setIsValid(isValidColor);
    if (isValidColor) {
      onChange(value);
    }
  };

  return (
    <div className="color-picker">
      <label htmlFor="color-input" className="color-picker-label">
        {label}
      </label>
      <div className="color-picker-input">
        <button
          type="button"
          className="color-preview-button"
          onClick={() => setIsOpen(!isOpen)}
          style={{ backgroundColor: color }}
          aria-label={`Current color: ${color}`}
        >
          <span className="color-value">{color}</span>
        </button>
        <input
          id="color-input"
          type="color"
          value={color}
          aria-label={label}
          onChange={(e) => handleColorChange(e.target.value)}
          className={`color-input ${isOpen ? 'visible' : ''} ${!isValid ? 'invalid' : ''
            }`}
        />
      </div>
      {!isValid && (
        <span className="error-message" role="alert">
          Please enter a valid hex color
        </span>
      )}
    </div>
  );
}
