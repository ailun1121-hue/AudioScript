import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@/shared/theme.css'
import { App } from './App'

const root = document.getElementById('root')
if (!root) throw new Error('#root 未找到')

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
