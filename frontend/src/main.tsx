import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { SimProvider } from './context/SimContext';
import { WebSocketProvider } from './context/WebSocketContext';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <WebSocketProvider>
      <SimProvider>
        <App />
      </SimProvider>
    </WebSocketProvider>
  </React.StrictMode>,
);
