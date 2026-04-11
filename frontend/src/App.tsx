import { Routes, Route, Navigate } from 'react-router-dom'
import LoginPage from './pages/LoginPage'
import RegisterPage from './pages/RegisterPage'
import SetupPage from './pages/SetupPage'
import TwoFactorPage from './pages/TwoFactorPage'
import AdminPage from './pages/AdminPage'
import ChatPage from './pages/ChatPage'

function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route path="/setup" element={<SetupPage />} />
      <Route path="/login/2fa" element={<TwoFactorPage />} />
      <Route path="/admin" element={<AdminPage />} />
      <Route path="/" element={<ChatPage />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  )
}

export default App
