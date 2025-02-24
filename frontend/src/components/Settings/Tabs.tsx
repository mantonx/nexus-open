import React, { useState, Children } from 'react';
import { TabProps, TabsProps, TabPanelProps } from './types';

function Tab({ label, isActive, onClick }: TabProps) {
  return (
    <button
      className={`tab-button ${isActive ? 'active' : ''}`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

export function Tabs({ children }: TabsProps) {
  const [activeTab, setActiveTab] = useState(0);

  return (
    <div className="tabs-container">
      <div className="tabs-header">
        {Children.map(children, (child, index) => (
          <Tab
            key={index}
            label={child.props.label}
            isActive={index === activeTab}
            onClick={() => setActiveTab(index)}
          />
        ))}
      </div>
      <div className="tab-content">
        {Children.toArray(children)[activeTab]}
      </div>
    </div>
  );
}
