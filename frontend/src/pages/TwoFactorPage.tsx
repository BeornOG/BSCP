import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { verify2fa } from "../hooks/useAuth";

export default function TwoFactorPage() {
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    const otp = (e.currentTarget.elements.namedItem("otp") as HTMLInputElement).value;

    try {
      const result = await verify2fa(otp);
      if (result.success) {
        navigate("/");
      } else {
        setError(result.error || "Invalid code. Please try again.");
      }
    } catch {
      setError("Network error. Please try again.");
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#121212] px-4">
      <div className="w-full max-w-sm bg-[#1e1e1e] border border-[#333] rounded-lg p-8 text-center">
        <h1 className="text-2xl font-bold text-white mb-2">2FA Code</h1>
        <p className="text-sm text-gray-400 mb-6">Enter the 6 digit code from your authenticator app</p>

        {error && (
          <div className="mb-4 p-3 bg-red-900/40 border border-red-700 rounded text-red-300 text-sm">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit}>
          <input
            name="otp"
            type="text"
            placeholder="000000"
            pattern="[0-9]*"
            inputMode="numeric"
            maxLength={6}
            required
            autoFocus
            className="w-full px-4 py-4 text-xl tracking-[0.3em] text-center bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 mb-4"
          />
          <button type="submit" className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded transition-colors">
            Submit
          </button>
        </form>
      </div>
    </div>
  );
}
