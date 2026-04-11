import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'

interface UserProfile {
  display_name: string
  theme: string
  accent_color: string
  profile_pic: string
}

export function useAuth(requireAuth = true) {
  const [user, setUser] = useState<UserProfile | null>(null)
  const [loading, setLoading] = useState(true)
  const navigate = useNavigate()

  useEffect(() => {
    async function checkAuth() {
      try {
        // Check if setup is needed first
        const setupRes = await fetch('/api/auth/setup')
        if (setupRes.ok) {
          const setupData = await setupRes.json()
          if (setupData.needs_setup) {
            navigate('/setup', { replace: true })
            setLoading(false)
            return
          }
        }

        if (!requireAuth) {
          setLoading(false)
          return
        }

        const res = await fetch('/api/userprofile/')
        if (res.status === 401) {
          navigate('/login', { replace: true })
          return
        }
        if (res.ok) {
          setUser(await res.json())
        }
      } catch {
        // Network error - stay on current page
      } finally {
        setLoading(false)
      }
    }
    checkAuth()
  }, [requireAuth, navigate])

  return { user, loading }
}

export async function login(username: string, password: string): Promise<{ success: boolean; requires_2fa?: boolean; error?: string }> {
  const res = await fetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user: username, password }),
  })
  return res.json()
}

export async function verify2fa(otp: string): Promise<{ success?: boolean; error?: string }> {
  const res = await fetch('/api/auth/2fa', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ otp }),
  })
  return res.json()
}

export async function setup(data: { username: string; email?: string; password: string; password_confirm: string }): Promise<{ success?: boolean; errors?: string[] }> {
  const res = await fetch('/api/auth/setup', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  return res.json()
}

export async function register(data: { username: string; password: string; password_confirm: string; invite_code: string }): Promise<{ success?: boolean; errors?: string[] }> {
  const res = await fetch('/api/auth/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  return res.json()
}

export async function logout(): Promise<void> {
  await fetch('/api/auth/logout', { method: 'POST' })
}
