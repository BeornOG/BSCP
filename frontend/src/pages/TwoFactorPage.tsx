import { useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import AuthLayout from '../components/layout/AuthLayout';
import { Input, Button } from '../components/ui';
import { useVerify2fa } from '../hooks/useAuth';

export default function TwoFactorPage() {
  const navigate = useNavigate();
  const verify = useVerify2fa();

  const [otp, setOtp] = useState('');
  const [error, setError] = useState('');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError('');

    verify.mutate(
      { otp },
      {
        onSuccess: (data) => {
          if (data.success) {
            navigate('/');
          } else {
            setError(data.error || 'Invalid code');
          }
        },
        onError: () => setError('Network error. Please try again.'),
      }
    );
  };

  return (
    <AuthLayout title="Two-factor authentication" subtitle="Enter the code from your authenticator app">
      <form onSubmit={handleSubmit} className="space-y-4">
        {error && (
          <div className="rounded-md bg-red-500/10 border border-red-500/30 px-4 py-3 text-sm text-red-400">
            {error}
          </div>
        )}

        <Input
          value={otp}
          onChange={(e) => setOtp(e.target.value.replace(/\D/g, '').slice(0, 6))}
          placeholder="000000"
          className="text-center text-2xl tracking-widest"
          maxLength={6}
          autoFocus
          required
        />

        <Button type="submit" className="w-full" disabled={verify.isPending || otp.length !== 6}>
          {verify.isPending ? 'Verifying...' : 'Verify'}
        </Button>
      </form>
    </AuthLayout>
  );
}
