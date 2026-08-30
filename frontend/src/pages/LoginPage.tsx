import { useState, type FormEvent } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import AuthLayout from '../components/layout/AuthLayout';
import { Input, Button } from '../components/ui';
import { useAuthCheck, useLogin } from '../hooks/useAuth';

export default function LoginPage() {
  const navigate = useNavigate();
  const { data: auth, isLoading } = useAuthCheck(false);
  const login = useLogin();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  const nextParam = new URLSearchParams(window.location.search).get('next');
  const safeNext = nextParam && nextParam.startsWith('/') && !nextParam.startsWith('//') ? nextParam : null;

  if (isLoading) return null;

  if (auth?.needsSetup) {
    navigate('/setup');
    return null;
  }

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError('');
    login.mutate(
      { user: username, password },
      {
        onSuccess: (data) => {
          if (data.success) {
            if (safeNext) window.location.assign(safeNext);
            else navigate('/');
          } else if (data.requires_2fa) {
            navigate('/login/2fa' + (safeNext ? `?next=${encodeURIComponent(safeNext)}` : ''));
          } else {
            setError(data.error || 'Invalid username or password.');
          }
        },
        onError: () => setError('Network error. Please try again.'),
      }
    );
  };

  return (
    <AuthLayout title="Welcome back" subtitle="Sign in to continue">
      <form onSubmit={handleSubmit} className="space-y-4">
        {error && (
          <div className="rounded-md bg-red-500/10 border border-red-500/30 px-4 py-3 text-sm text-red-400">
            {error}
          </div>
        )}

        <Input
          placeholder="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          required
        />

        <Input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          required
        />

        <Button type="submit" className="w-full" disabled={login.isPending}>
          {login.isPending ? 'Signing in...' : 'Sign in'}
        </Button>
      </form>

      <p className="mt-6 text-center text-sm text-[#71747a]">
        Don't have an account?{' '}
        <Link to="/register" className="text-[var(--accent)] hover:underline">
          Register
        </Link>
      </p>
    </AuthLayout>
  );
}
