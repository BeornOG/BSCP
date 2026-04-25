import { useState, useEffect } from 'react';
import { useAdminConfig, useUpdateAdminConfig } from '../../hooks/useAdminConfig';
import { useQuery } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { Button, Input } from '../ui';
import UserStorageManager from './UserStorageManager';
import { formatStorageMB } from '../../lib/format';

interface UserList {
  users: Array<{ id: string; username: string; storage_limit_mb: number }>;
}

export default function StorageSettings() {
  const { data: config, isLoading } = useAdminConfig();
  const updateConfig = useUpdateAdminConfig();
  const { data: userList, isLoading: usersLoading } = useQuery<UserList>({
    queryKey: ['users:storage'],
    queryFn: () => api<UserList>('/api/users'),
  });
  const [storageLimit, setStorageLimit] = useState('500');

  useEffect(() => {
    if (config) {
      setStorageLimit(config.storage_limit_mb.toString());
    }
  }, [config]);

  const handleUpdate = () => {
    const limit = parseInt(storageLimit);
    if (limit < 1) {
      alert('Storage limit must be at least 1 MB');
      return;
    }
    updateConfig.mutate({ storage_limit_mb: limit });
  };

  if (isLoading) return <p>Loading settings...</p>;

  return (
    <div className="space-y-6">
      <div>
        <h3 className="font-semibold mb-4">Default Storage Limit</h3>
        <p className="text-xs text-gray-400 mb-3">
          Default limit applied to new users. Existing users can have individual limits set.
        </p>
        <div className="flex gap-2">
          <div className="flex-1">
            <Input
              type="number"
              min="1"
              value={storageLimit}
              onChange={(e) => setStorageLimit(e.target.value)}
            />
            {storageLimit && (
              <p className="text-xs text-gray-400 mt-1">
                = {formatStorageMB(parseInt(storageLimit))}
              </p>
            )}
          </div>
          <Button
            onClick={handleUpdate}
            disabled={updateConfig.isPending}
          >
            {updateConfig.isPending ? 'Saving...' : 'Save'}
          </Button>
        </div>

        {updateConfig.isSuccess && (
          <p className="text-sm text-green-400 mt-2">Updated!</p>
        )}
        {updateConfig.isError && (
          <p className="text-sm text-red-400 mt-2">Failed to update</p>
        )}
      </div>

      {userList && userList.users && (
        <div className="border-t border-[#232529] pt-4">
          <UserStorageManager
            users={userList.users}
            isLoading={usersLoading}
          />
        </div>
      )}
    </div>
  );
}
