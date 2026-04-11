import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { register, useAuth } from "../hooks/useAuth";

export default function RegisterPage() {
  const [errors, setErrors] = useState<string[]>([]);
  const navigate = useNavigate();
  const { loading } = useAuth(false);

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setErrors([]);

    const form = e.currentTarget;
    const username = (form.elements.namedItem("username") as HTMLInputElement).value;
    const password = (form.elements.namedItem("password") as HTMLInputElement).value;
    const password_confirm = (form.elements.namedItem("password_confirm") as HTMLInputElement).value;
    const invite_code = (form.elements.namedItem("invite_code") as HTMLInputElement).value;

    if (password !== password_confirm) {
      setErrors(["Passwords do not match."]);
      return;
    }

    try {
      const result = await register({ username, password, password_confirm, invite_code });
      if (result.success) {
        navigate("/login");
      } else {
        setErrors(result.errors || ["Registration failed. Please try again."]);
      }
    } catch {
      setErrors(["Network error. Please try again."]);
    }
  }

  if (loading) return null;

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#121212] px-4">
      <div className="w-full max-w-md bg-[#1e1e1e] border border-[#333] rounded-lg p-8">
        <h1 className="text-2xl font-bold text-white text-center mb-1">Create Account</h1>
        <p className="text-sm text-gray-400 text-center mb-6">Enter your invite code</p>

        {errors.length > 0 && (
          <div className="mb-4 p-3 bg-red-900/40 border border-red-700 rounded text-red-300 text-sm">
            <ul className="list-disc list-inside">
              {errors.map((err, i) => <li key={i}>{err}</li>)}
            </ul>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="username" className="block text-sm font-medium text-gray-300 mb-1">Username</label>
            <input type="text" id="username" name="username" required autoFocus
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label htmlFor="password" className="block text-sm font-medium text-gray-300 mb-1">Password</label>
            <input type="password" id="password" name="password" required
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label htmlFor="password_confirm" className="block text-sm font-medium text-gray-300 mb-1">Confirm Password</label>
            <input type="password" id="password_confirm" name="password_confirm" required
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label htmlFor="invite_code" className="block text-sm font-medium text-gray-300 mb-1">Invite Code</label>
            <input type="text" id="invite_code" name="invite_code" required
              className="w-full px-3 py-2 bg-[#2a2a2a] border border-[#333] rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500" />
          </div>
          <button type="submit" className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded transition-colors">
            Create Account
          </button>
        </form>

        <p className="mt-4 text-center text-sm text-gray-400">
          Already have an account?{" "}
          <Link to="/login" className="text-blue-400 hover:text-blue-300">Log in</Link>
        </p>
      </div>
    </div>
  );
}
