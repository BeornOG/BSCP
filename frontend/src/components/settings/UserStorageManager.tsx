import { useState } from 'react';
import { useUpdateUserStorageLimit } from '../../hooks/useUserStorageConfig';
import { Button, Input } from '../ui';
import { formatStorageMB } from '../../lib/format';

interface User {
  id: string;
  username: string;
  storage_limit_mb: number;
}

interface UserStorageManagerProps {
  users: User[];
  isLoading?: boolean;
}

export default function UserStorageManager({ users, isLoading }: UserStorageManagerProps) {
  const [selectedUserId, setSelectedUserId] = useState<string | null>(null);
  const [storageLimit, setStorageLimit] = useState('');
  const updateLimit = useUpdateUserStorageLimit();

  const selectedUser = selectedUserId ? users.find(u => u.id === selectedUserId) : null;

  const handleSelectUser = (userId: string) => {
    const user = users.find(u => u.id === userId);
    if (user) {
      setSelectedUserId(userId);
      setStorageLimit(user.storage_limit_mb.toString());
    }
  };

  const handleUpdate = () => {
    if (!selectedUserId || !storageLimit) return;
    const limit = parseInt(storageLimit);
    if (limit < 1) {
      alert('Storage limit must be at least 1 MB');
      return;
    }
    updateLimit.mutate({ username: selectedUserId, limit_mb: limit });
  };

  if (isLoading) return <p>Loading users...</p>;
  if (users.length === 0) return <p className="text-gray-400">No users found</p>;

  return (
    <div className="space-y-4">
      <h3 className="font-semibold">Manage User Storage Limits</h3>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium mb-2">Select User</label>
          <select
            value={selectedUserId || ''}
            onChange={(e) => handleSelectUser(e.target.value)}
            className="w-full px-3 py-2 rounded border border-[#232529] bg-[#141517] text-sm text-[#e8eaed]"
          >
            <option value="">Choose a user...</option>
            {users.map((user) => (
              <option key={user.id} value={user.id}>
                {user.username}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="block text-sm font-medium mb-2">Storage Limit (MB)</label>
          <div className="flex gap-2">
            <div className="flex-1">
              <Input
                type="number"
                min="1"
                value={storageLimit}
                onChange={(e) => setStorageLimit(e.target.value)}
                disabled={!selectedUserId}
              />
              {storageLimit && (
                <p className="text-xs text-gray-400 mt-1">
                  = {formatStorageMB(parseInt(storageLimit))}
                </p>
              )}
            </div>
            <Button
              onClick={handleUpdate}
              disabled={!selectedUserId || updateLimit.isPending}
            >
              {updateLimit.isPending ? 'Saving...' : 'Set'}
            </Button>
          </div>
        </div>
      </div>

      {selectedUser && (
        <div className="p-3 rounded bg-blue-600/10 border border-blue-600/30">
          <p className="text-sm text-blue-300">
            {selectedUser.username}: {selectedUser.storage_limit_mb} MB ({formatStorageMB(selectedUser.storage_limit_mb)})
          </p>
        </div>
      )}

      {updateLimit.isSuccess && (
        <p className="text-sm text-green-400">Limit updated successfully!</p>
      )}
      {updateLimit.isError && (
        <p className="text-sm text-red-400">Failed to update limit</p>
      )}
    </div>
  );
}
