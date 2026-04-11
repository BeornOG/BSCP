import { useUsers, useInvites, useGenerateInvite, useDeleteUser } from '../hooks/useAdmin';
import { Card, CardHeader, CardContent, Button, Badge, Spinner } from '../components/ui';

export default function AdminPage() {
  const { data: users, isLoading: usersLoading } = useUsers();
  const { data: invites, isLoading: invitesLoading } = useInvites();
  const generateInvite = useGenerateInvite();
  const deleteUser = useDeleteUser();

  const handleDelete = (fullId: string) => {
    if (window.confirm(`Are you sure you want to deactivate user "${fullId}"?`)) {
      deleteUser.mutate(fullId);
    }
  };

  return (
    <div className="max-w-4xl mx-auto px-6 py-10 space-y-8">
      <h1 className="text-2xl font-semibold text-[#e8eaed]">Administration</h1>

      {/* Invite Codes */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between w-full">
            <h2 className="text-lg font-medium text-[#e8eaed]">Invite Codes</h2>
            <Button onClick={() => generateInvite.mutate()} disabled={generateInvite.isPending}>
              {generateInvite.isPending ? 'Generating...' : 'Generate'}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {invitesLoading ? (
            <Spinner />
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-[#71747a]">
                  <th className="pb-3 font-medium">Code</th>
                  <th className="pb-3 font-medium">Status</th>
                  <th className="pb-3 font-medium">Expires</th>
                  <th className="pb-3 font-medium">Used by</th>
                </tr>
              </thead>
              <tbody>
                {invites?.map((invite) => (
                  <tr key={invite.code} className="border-b border-[#232529] last:border-0">
                    <td className="py-3 font-mono text-[#e8eaed]">{invite.code}</td>
                    <td className="py-3">
                      <Badge variant={invite.status === 'active' ? 'success' : 'default'}>
                        {invite.status}
                      </Badge>
                    </td>
                    <td className="py-3 text-[#71747a]">
                      {new Date(invite.expires_at * 1000).toLocaleDateString()}
                    </td>
                    <td className="py-3 text-[#71747a]">{invite.used_by || '-'}</td>
                  </tr>
                ))}
                {invites?.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-6 text-center text-[#71747a]">
                      No invite codes generated yet
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>

      {/* User Management */}
      <Card>
        <CardHeader>
          <h2 className="text-lg font-medium text-[#e8eaed]">User Management</h2>
        </CardHeader>
        <CardContent>
          {usersLoading ? (
            <Spinner />
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-[#71747a]">
                  <th className="pb-3 font-medium">Username</th>
                  <th className="pb-3 font-medium">Display Name</th>
                  <th className="pb-3 font-medium">Status</th>
                  <th className="pb-3 font-medium text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {users?.map((user) => (
                  <tr key={user.username} className="border-b border-[#232529] last:border-0">
                    <td className="py-3 text-[#e8eaed]">{user.username}</td>
                    <td className="py-3 text-[#71747a]">{user.display_name}</td>
                    <td className="py-3">
                      <Badge variant={user.status === 'online' ? 'success' : 'default'}>
                        {user.status}
                      </Badge>
                    </td>
                    <td className="py-3 text-right">
                      <button
                        onClick={() => handleDelete(user.username)}
                        className="text-red-400 hover:text-red-300 text-xs transition-colors"
                        disabled={deleteUser.isPending}
                      >
                        Deactivate
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
