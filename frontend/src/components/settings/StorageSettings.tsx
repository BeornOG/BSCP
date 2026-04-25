import { useState, useEffect } from 'react';
import { useAdminConfig, useUpdateAdminConfig } from '../../hooks/useAdminConfig';
import { Button, Input } from '../ui';

export default function StorageSettings() {
  const { data: config, isLoading } = useAdminConfig();
  const updateConfig = useUpdateAdminConfig();
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
    <div className="space-y-4">
      <h3 className="font-semibold">Storage Configuration (Admin)</h3>

      <div>
        <label className="block text-sm font-medium mb-2">
          Per-User Storage Limit (MB)
        </label>
        <div className="flex gap-2">
          <Input
            type="number"
            min="1"
            value={storageLimit}
            onChange={(e) => setStorageLimit(e.target.value)}
            className="flex-1"
          />
          <Button
            onClick={handleUpdate}
            disabled={updateConfig.isPending}
          >
            {updateConfig.isPending ? 'Saving...' : 'Save'}
          </Button>
        </div>
        <p className="text-xs text-gray-400 mt-2">
          Controls maximum storage per user across all uploads.
        </p>
      </div>

      {updateConfig.isSuccess && (
        <p className="text-sm text-green-400">Settings updated successfully!</p>
      )}
      {updateConfig.isError && (
        <p className="text-sm text-red-400">Failed to update settings</p>
      )}
    </div>
  );
}
