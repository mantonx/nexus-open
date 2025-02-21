import './App.css'

import { UpdateConfig } from '../wailsjs/go/main/App'
import { useState } from 'react'

function App() {
  const [selectedColor, setSelectedColor] = useState('#000000')
  const [units, setUnits] = useState('metric')

  const handleSubmit = async () => {
    try {
      await UpdateConfig({ HexColor: selectedColor, Unit: units })
    } catch (error) {
      console.error('Error updating color:', error)
    }
  }
  return (
    <div className="app">
      <div>
        <label htmlFor="units">Units: </label>
        <select id="units" value={units} onChange={(e) => setUnits(e.target.value)}>
          <option value="metric">Metric</option>
          <option value="imperial">Imperial</option>
        </select>
      </div>
      <div>
        <label htmlFor="colorPicker">Choose a color: </label>
        <input
          type="color"
          id="colorPicker"
          value={selectedColor}
          onChange={(e) => setSelectedColor(e.target.value)}
        />
      </div>
      <button type="button" onClick={handleSubmit}>
        Update Color
      </button>
      <p>Selected color: {selectedColor}</p>
    </div>
  )
}

export default App
