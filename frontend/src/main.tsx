import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import './index.css'
import App from './App'

// Apply saved theme/accent from localStorage before first paint
const savedTheme = localStorage.getItem('theme') || 'dark'
document.body.classList.add(`theme-${savedTheme}`)
const savedAccent = localStorage.getItem('accent_color')
if (savedAccent) document.documentElement.style.setProperty('--accent', savedAccent)

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      refetchOnWindowFocus: false,
    },
  },
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>,
)
