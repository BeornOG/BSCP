import { useState, useEffect } from "react";

interface Invite {
  code: string;
  status: string;
  expires_at: number;
  used_by: string | null;
}

interface User {
  id: number;
  username: string;
  is_admin: boolean;
  is_2fa: boolean;
}

export default function AdminPage() {
  const [invites, setInvites] = useState<Invite[]>([]);
  const [users, setUsers] = useState<User[]>([]);

  const fetchInvites = () => {
    fetch("/api/invites")
      .then((res) => res.json())
      .then((data) => setInvites(data))
      .catch(console.error);
  };

  const fetchUsers = () => {
    fetch("/api/users")
      .then((res) => res.json())
      .then((data) => setUsers(data))
      .catch(console.error);
  };

  useEffect(() => {
    fetchInvites();
    fetchUsers();
  }, []);

  const handleGenerate = () => {
    fetch("/api/invites/generate", { method: "POST" })
      .then(() => fetchInvites())
      .catch(console.error);
  };

  const handleDelete = (id: number) => {
    if (!confirm(`Are you sure you want to delete user ${id}?`)) return;
    fetch(`/api/users/${id}`, { method: "DELETE" })
      .then(() => fetchUsers())
      .catch(console.error);
  };

  const formatExpiry = (unix: number) => {
    if (!unix) return "—";
    return new Date(unix * 1000).toLocaleString();
  };

  return (
    <div className="min-h-screen bg-[#0c0f10] text-[#f8f9fc] p-8">
      {/* Header */}
      <div className="mb-10">
        <h1 className="text-3xl font-bold">Admin Dashboard</h1>
        <p className="text-sm text-gray-400 mt-1">
          Manage system invites and user permissions.
        </p>
      </div>

      {/* Invite Codes */}
      <div className="bg-[#1c2023] border border-[#222629] rounded-lg mb-8">
        <div className="flex items-center justify-between px-6 py-4 border-b border-[#222629]">
          <h2 className="text-lg font-semibold">Invite Codes</h2>
          <button
            onClick={handleGenerate}
            className="flex items-center gap-1.5 px-4 py-2 rounded-md text-sm font-medium bg-[var(--dynamic-primary,#7eafff)] text-[#0c0f10] hover:opacity-90 transition-opacity"
          >
            <span className="material-symbols-outlined text-[18px]">add</span>
            Generate Code
          </button>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b border-[#222629]">
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Code
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Status
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Expires
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Used By ID
                </th>
              </tr>
            </thead>
            <tbody>
              {invites.map((inv) => (
                <tr
                  key={inv.code}
                  className="border-b border-[#222629] last:border-0 hover:bg-[#222629]/40"
                >
                  <td className="px-6 py-3 text-sm font-mono">{inv.code}</td>
                  <td className="px-6 py-3 text-sm">{inv.status}</td>
                  <td className="px-6 py-3 text-sm">
                    {formatExpiry(inv.expires_at)}
                  </td>
                  <td className="px-6 py-3 text-sm">
                    {inv.used_by ?? "—"}
                  </td>
                </tr>
              ))}
              {invites.length === 0 && (
                <tr>
                  <td
                    colSpan={4}
                    className="px-6 py-6 text-center text-sm text-gray-500"
                  >
                    No invite codes found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* User Management */}
      <div className="bg-[#1c2023] border border-[#222629] rounded-lg">
        <div className="px-6 py-4 border-b border-[#222629]">
          <h2 className="text-lg font-semibold">User Management</h2>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b border-[#222629]">
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  ID
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Username
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Role
                </th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wider text-gray-400">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr
                  key={u.id}
                  className="border-b border-[#222629] last:border-0 hover:bg-[#222629]/40"
                >
                  <td className="px-6 py-3 text-sm">{u.id}</td>
                  <td className="px-6 py-3 text-sm">{u.username}</td>
                  <td className="px-6 py-3 text-sm">
                    {u.is_admin ? "Admin" : "User"}
                  </td>
                  <td className="px-6 py-3 text-sm">
                    {!u.is_admin && (
                      <button
                        onClick={() => handleDelete(u.id)}
                        className="flex items-center gap-1 text-red-400 hover:text-red-300 transition-colors"
                      >
                        <span className="material-symbols-outlined text-[18px]">
                          delete
                        </span>
                        Delete
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {users.length === 0 && (
                <tr>
                  <td
                    colSpan={4}
                    className="px-6 py-6 text-center text-sm text-gray-500"
                  >
                    No users found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
