import { useState } from 'react';
import { useUsers, useInvites, useGenerateInvite, useDeleteUser } from '../hooks/useAdmin';
import { useUpdateUserStorageLimit } from '../hooks/useUserStorageConfig';
import { useOAuthConfig, useSetOAuthEnabled, useOAuthClients, useRevokeOAuthClient } from '../hooks/useOAuthApps';
import { useModules, useAddModule, useSetModuleEnabled, useRemoveModule, type AddModuleResult } from '../hooks/useModules';
import { useBlockedDomains, useBlockDomain, useUnblockDomain } from '../hooks/useBlockedDomains';
import { Card, CardHeader, CardContent, Button, Badge, Spinner, Input } from '../components/ui';

export default function AdminPage() {
  const { data: users, isLoading: usersLoading } = useUsers();
  const { data: invites, isLoading: invitesLoading } = useInvites();
  const generateInvite = useGenerateInvite();
  const deleteUser = useDeleteUser();
  const updateStorage = useUpdateUserStorageLimit();

  const [selectedUsername, setSelectedUsername] = useState<string | null>(null);
  const [storageInput, setStorageInput] = useState('');

  const oauthConfig = useOAuthConfig();
  const setOAuthEnabled = useSetOAuthEnabled();
  const { data: oauthClients } = useOAuthClients();
  const revokeClient = useRevokeOAuthClient();

  const { data: modules } = useModules();
  const addModule = useAddModule();
  const setModuleEnabled = useSetModuleEnabled();
  const removeModule = useRemoveModule();
  const [moduleUrl, setModuleUrl] = useState('');
  const [newModule, setNewModule] = useState<AddModuleResult | null>(null);

  const { data: blockedDomains } = useBlockedDomains();
  const blockDomain = useBlockDomain();
  const unblockDomain = useUnblockDomain();
  const [domainInput, setDomainInput] = useState('');
  const [domainReason, setDomainReason] = useState('');

  const handleBlockDomain = () => {
    if (!domainInput.trim()) return;
    blockDomain.mutate(
      { domain: domainInput.trim(), reason: domainReason.trim() || undefined },
      {
        onSuccess: () => {
          setDomainInput('');
          setDomainReason('');
        },
      },
    );
  };

  const handleAddModule = () => {
    if (!moduleUrl.trim()) return;
    addModule.mutate(moduleUrl.trim(), {
      onSuccess: (res) => {
        setNewModule(res);
        setModuleUrl('');
      },
    });
  };

  const selectedUser = selectedUsername ? users?.find(u => u.username === selectedUsername) : null;

  const handleDelete = (fullId: string) => {
    if (window.confirm(`Are you sure you want to deactivate user "${fullId}"?`)) {
      deleteUser.mutate(fullId);
    }
  };

  const openStorageDialog = (user: any) => {
    setSelectedUsername(user.username);
    setStorageInput((user.storage_limit_mb || 500).toString());
  };

  const handleSaveStorage = () => {
    if (!selectedUsername) return;
    const limit = parseInt(storageInput);
    if (limit < 1) {
      alert('Storage limit must be at least 1 MB');
      return;
    }
    updateStorage.mutate({ username: selectedUsername, limit_mb: limit }, {
      onSuccess: () => {
        setSelectedUsername(null);
        setStorageInput('');
      },
    });
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
                      {invite.expires_at ? new Date(invite.expires_at * 1000).toLocaleDateString() : 'Never'}
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
                    <td className="py-3 text-right space-x-2">
                      <button
                        onClick={() => openStorageDialog(user)}
                        className="text-blue-400 hover:text-blue-300 text-xs transition-colors"
                      >
                        Storage
                      </button>
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

      {/* OAuth Applications */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between w-full">
            <h2 className="text-lg font-medium text-[#e8eaed]">OAuth Applications</h2>
            <label className="flex items-center gap-2 text-sm text-[#71747a]">
              <input
                type="checkbox"
                checked={oauthConfig.data?.oidc_enabled ?? true}
                onChange={(e) => setOAuthEnabled.mutate(e.target.checked)}
              />
              Sign in with BSCP enabled
            </label>
          </div>
        </CardHeader>
        <CardContent>
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-[#71747a]">
                <th className="pb-3 font-medium">Name</th>
                <th className="pb-3 font-medium">Client ID</th>
                <th className="pb-3 font-medium">Redirect URIs</th>
                <th className="pb-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {oauthClients?.map((c) => (
                <tr key={c.client_id} className="border-b border-[#232529] last:border-0">
                  <td className="py-3 text-[#e8eaed]">{c.name || '—'}</td>
                  <td className="py-3 font-mono text-xs text-[#71747a]">{c.client_id}</td>
                  <td className="py-3 text-xs text-[#71747a] break-all">{c.redirect_uris.join(', ')}</td>
                  <td className="py-3 text-right">
                    <button
                      onClick={() => revokeClient.mutate(c.client_id)}
                      className="text-red-400 hover:text-red-300 text-xs"
                    >
                      Revoke
                    </button>
                  </td>
                </tr>
              ))}
              {(!oauthClients || oauthClients.length === 0) && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-[#71747a]">
                    No applications have registered yet
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </CardContent>
      </Card>

      {/* Modules */}
      <Card>
        <CardHeader>
          <h2 className="text-lg font-medium text-[#e8eaed]">Modules</h2>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2 mb-4">
            <Input
              placeholder="https://module.example"
              value={moduleUrl}
              onChange={(e) => setModuleUrl(e.target.value)}
            />
            <Button onClick={handleAddModule} disabled={addModule.isPending}>
              {addModule.isPending ? 'Adding…' : 'Add'}
            </Button>
          </div>
          {addModule.isError && (
            <p className="text-sm text-red-400 mb-3">{(addModule.error as Error).message}</p>
          )}
          {newModule && (
            <div className="mb-4 rounded-md bg-[#1a1d21] border border-[#232529] p-3 text-sm">
              <p className="text-[#e8eaed] font-medium">{newModule.name} added.</p>
              <p className="text-[#71747a] mt-1">
                Shared secret (shown once — configure it in the module):
              </p>
              <code className="block mt-1 break-all text-[#7eafff]">{newModule.secret}</code>
              {newModule.events.length > 0 && (
                <p className="text-[#71747a] mt-2">
                  Will receive events: {newModule.events.join(', ')}
                </p>
              )}
              <button onClick={() => setNewModule(null)} className="text-xs text-[#71747a] mt-2 underline">
                dismiss
              </button>
            </div>
          )}
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-[#71747a]">
                <th className="pb-3 font-medium">Name</th>
                <th className="pb-3 font-medium">URL</th>
                <th className="pb-3 font-medium">Events / Providers</th>
                <th className="pb-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {modules?.map((m) => (
                <tr key={m.name} className="border-b border-[#232529] last:border-0">
                  <td className="py-3 text-[#e8eaed]">{m.name}</td>
                  <td className="py-3 text-xs text-[#71747a] break-all">{m.base_url}</td>
                  <td className="py-3 text-xs text-[#71747a]">
                    {m.manifest.events.join(', ')}
                    {m.manifest.link_providers.length > 0 &&
                      ` · links: ${m.manifest.link_providers.map((p) => p.name || p.id).join(', ')}`}
                  </td>
                  <td className="py-3 text-right space-x-2">
                    <button
                      onClick={() => setModuleEnabled.mutate({ name: m.name, enabled: !m.enabled })}
                      className="text-blue-400 hover:text-blue-300 text-xs"
                    >
                      {m.enabled ? 'Disable' : 'Enable'}
                    </button>
                    <button
                      onClick={() => removeModule.mutate(m.name)}
                      className="text-red-400 hover:text-red-300 text-xs"
                    >
                      Remove
                    </button>
                  </td>
                </tr>
              ))}
              {(!modules || modules.length === 0) && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-[#71747a]">
                    No modules installed
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </CardContent>
      </Card>

      {/* Blocked Domains */}
      <Card>
        <CardHeader>
          <h2 className="text-lg font-medium text-[#e8eaed]">Blocked Domains</h2>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-[#71747a] mb-4">
            Messages to and from a blocked domain are refused, and its channel servers
            can't be used from this server.
          </p>
          <div className="flex flex-wrap gap-2 mb-4">
            <Input
              placeholder="spam.example"
              value={domainInput}
              onChange={(e) => setDomainInput(e.target.value)}
            />
            <Input
              placeholder="Reason (optional)"
              value={domainReason}
              onChange={(e) => setDomainReason(e.target.value)}
            />
            <Button onClick={handleBlockDomain} disabled={blockDomain.isPending}>
              {blockDomain.isPending ? 'Blocking…' : 'Block'}
            </Button>
          </div>
          {blockDomain.isError && (
            <p className="text-sm text-red-400 mb-3">{(blockDomain.error as Error).message}</p>
          )}
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-[#71747a]">
                <th className="pb-3 font-medium">Domain</th>
                <th className="pb-3 font-medium">Reason</th>
                <th className="pb-3 font-medium">Blocked by</th>
                <th className="pb-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {blockedDomains?.map((d) => (
                <tr key={d.domain} className="border-b border-[#232529] last:border-0">
                  <td className="py-3 text-[#e8eaed]">{d.domain}</td>
                  <td className="py-3 text-[#71747a]">{d.reason || '—'}</td>
                  <td className="py-3 text-[#71747a]">{d.blocked_by || '—'}</td>
                  <td className="py-3 text-right">
                    <button
                      onClick={() => unblockDomain.mutate(d.domain)}
                      className="text-blue-400 hover:text-blue-300 text-xs transition-colors"
                    >
                      Unblock
                    </button>
                  </td>
                </tr>
              ))}
              {(!blockedDomains || blockedDomains.length === 0) && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-[#71747a]">
                    No domains blocked
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </CardContent>
      </Card>

      {/* Storage Management Modal */}
      {selectedUsername && selectedUser && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#141517] rounded-lg border border-[#232529] p-6 max-w-sm w-full mx-4 space-y-4">
            <h3 className="text-lg font-semibold text-[#e8eaed]">Manage Storage Limit</h3>
            <p className="text-sm text-[#71747a]">User: {selectedUser.username}</p>

            <div>
              <label className="block text-sm font-medium mb-2 text-[#e8eaed]">
                Storage Limit (MB)
              </label>
              <Input
                type="number"
                min="1"
                value={storageInput}
                onChange={(e) => setStorageInput(e.target.value)}
              />
            </div>

            <div className="flex gap-3 justify-end">
              <Button
                onClick={() => {
                  setSelectedUsername(null);
                  setStorageInput('');
                }}
                className="bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
              >
                Cancel
              </Button>
              <Button
                onClick={handleSaveStorage}
                disabled={updateStorage.isPending}
                className="bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
              >
                {updateStorage.isPending ? 'Saving...' : 'Save'}
              </Button>
            </div>

            {updateStorage.isSuccess && (
              <p className="text-sm text-green-400">Updated successfully!</p>
            )}
            {updateStorage.isError && (
              <p className="text-sm text-red-400">Failed to update storage limit</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
