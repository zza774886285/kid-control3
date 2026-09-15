import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import PointsPage from './components/PointsPage'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <PointsPage />
  </StrictMode>,
)
