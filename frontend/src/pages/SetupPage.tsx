import { useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import AuthLayout from '../components/layout/AuthLayout';
import { Input, Button } from '../components/ui';
import { useSetup } from '../hooks/useAuth';

export default function SetupPage() {
  const navigate = useNavigate();
  const setup = useSetup();

  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [errors, setErrors] = useState<string[]>([]);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setErrors([]);

    if (password !== confirmPassword) {
      setErrors(['Passwords do not match']);
      return;
    }

    setup.mutate(
      { username, email: email || undefined, password, password_confirm: confirmPassword },
      {
        onSuccess: (data) => {
          if (data.success) {
            navigate('/login');
          } else if (data.errors) {
            setErrors(data.errors);
          }
        },
        onError: () => setErrors(['Network error. Please try again.']),
      }
    );
  };

  return (
    <AuthLayout title="Welcome to Atelier" subtitle="Create your admin account to get started">
      <form onSubmit={handleSubmit} className="space-y-4">
        {errors.length > 0 && (
          <div className="rounded-md bg-red-500/10 border border-red-500/30 px-4 py-3 text-sm text-red-400 space-y-1">
            {errors.map((err, i) => (
              <p key={i}>{err}</p>
            ))}
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
          type="email"
          placeholder="Email (optional)"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="email"
        />

        <Input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
          required
        />

        <Input
          type="password"
          placeholder="Confirm password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          autoComplete="new-password"
          required
        />

        <Button type="submit" className="w-full" disabled={setup.isPending}>
          {setup.isPending ? 'Setting up...' : 'Create admin account'}
        </Button>
      </form>
    </AuthLayout>
  );
}
