import { useState, type FormEvent } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import AuthLayout from '../components/layout/AuthLayout';
import { Input, Button } from '../components/ui';
import { useRegister } from '../hooks/useAuth';

export default function RegisterPage() {
  const navigate = useNavigate();
  const register = useRegister();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [inviteCode, setInviteCode] = useState('');
  const [errors, setErrors] = useState<string[]>([]);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setErrors([]);

    if (password !== confirmPassword) {
      setErrors(['Passwords do not match']);
      return;
    }

    register.mutate(
      { username, password, password_confirm: confirmPassword, invite_code: inviteCode },
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
    <AuthLayout title="Create account" subtitle="Enter your invite code to get started">
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

        <Input
          placeholder="Invite code"
          value={inviteCode}
          onChange={(e) => setInviteCode(e.target.value)}
          required
        />

        <Button type="submit" className="w-full" disabled={register.isPending}>
          {register.isPending ? 'Creating account...' : 'Create account'}
        </Button>
      </form>

      <p className="mt-6 text-center text-sm text-[#71747a]">
        Already have an account?{' '}
        <Link to="/login" className="text-[var(--accent)] hover:underline">
          Sign in
        </Link>
      </p>
    </AuthLayout>
  );
}
