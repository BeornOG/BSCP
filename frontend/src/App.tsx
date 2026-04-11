import { Routes, Route, Navigate } from 'react-router-dom'
import AppLayout from './components/layout/AppLayout'
import LoginPage from './pages/LoginPage'
import RegisterPage from './pages/RegisterPage'
import SetupPage from './pages/SetupPage'
import TwoFactorPage from './pages/TwoFactorPage'
import AdminPage from './pages/AdminPage'
import ChatPage from './pages/ChatPage'
import SettingsPage from './pages/SettingsPage'

function App() {
  return (
    <Routes>
      {/* Auth routes - no app shell */}
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route path="/setup" element={<SetupPage />} />
      <Route path="/login/2fa" element={<TwoFactorPage />} />

      {/* App routes - wrapped in AppLayout */}
      <Route path="/" element={<AppLayout><ChatPage /></AppLayout>} />
      <Route path="/admin" element={<AppLayout><AdminPage /></AppLayout>} />
      <Route path="/settings" element={<AppLayout><SettingsPage /></AppLayout>} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  )
}

export default App
