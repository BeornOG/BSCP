import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { login, useAuth } from "../hooks/useAuth";

export default function LoginPage() {
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();
  const { loading } = useAuth(false);

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    const form = e.currentTarget;
    const username = (form.elements.namedItem("user") as HTMLInputElement).value;
    const password = (form.elements.namedItem("password") as HTMLInputElement).value;

    try {
      const result = await login(username, password);
      if (result.success) {
        navigate("/");
      } else if (result.requires_2fa) {
        navigate("/login/2fa");
      } else {
        setError(result.error || "Invalid username or password.");
      }
    } catch {
      setError("Network error. Please try again.");
    }
  }

  if (loading) return null;

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#121212] px-4">
      <div className="w-full max-w-md bg-[#1e1e1e] border border-[#333] rounded-lg p-8">
        <h1 className="text-2xl font-bold text-white text-center mb-6">Login</h1>

        {error && (
          <div className="mb-4 p-3 bg-red-900/40 border border-red-700 rounded text-red-300 text-sm">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="user" className="block text-sm font-medium text-gray-300 mb-1">Username</label>
            <input type="text" id="user" name="user" required autoFocus
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label htmlFor="password" className="block text-sm font-medium text-gray-300 mb-1">Password</label>
            <input type="password" id="password" name="password" required
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <button type="submit" className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded transition-colors">
            Next
          </button>
        </form>

        <p className="mt-4 text-center text-sm text-gray-400">
          No account?{" "}
          <Link to="/register" className="text-blue-400 hover:text-blue-300">Register</Link>
        </p>
      </div>
    </div>
  );
}
