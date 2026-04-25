import { useUploads, useDeleteUpload } from '../../hooks/useUploads';
import { useProfile } from '../../hooks/useProfile';
import { Button } from '../ui';
import { formatBytes } from '../../lib/format';

const formatDate = (timestamp: number): string => {
  return new Date(timestamp * 1000).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
};

export default function MediaManager() {
  const { data: uploads, isLoading } = useUploads();
  const { data: profile } = useProfile();
  const deleteUpload = useDeleteUpload();

  if (isLoading) return <p>Loading uploads...</p>;
  if (!uploads) return <p>No uploads</p>;

  const { uploads: files, total_size_bytes, limit_bytes } = uploads;
  const isUnlimited = profile?.is_primary_admin;
  const percentUsed = isUnlimited ? 0 : Math.round((total_size_bytes / limit_bytes) * 100);
  const remainingBytes = isUnlimited ? 0 : limit_bytes - total_size_bytes;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="font-semibold mb-2">Storage Usage</h3>
        {isUnlimited ? (
          <div className="bg-green-600/20 border border-green-600/50 rounded-lg p-3">
            <p className="text-sm text-green-400 font-medium">Unlimited Storage</p>
            <p className="text-xs text-green-400/70">As primary admin, you have unlimited storage</p>
            <p className="text-sm text-gray-400 mt-2">Using: {formatBytes(total_size_bytes)}</p>
          </div>
        ) : (
          <>
            <div className="bg-gray-700 rounded-full h-2 overflow-hidden">
              <div
                className="bg-blue-500 h-full transition-all"
                style={{ width: `${percentUsed}%` }}
              />
            </div>
            <p className="text-sm text-gray-400 mt-2">
              {formatBytes(total_size_bytes)} / {formatBytes(limit_bytes)} ({percentUsed}%)
            </p>
            <p className="text-sm text-gray-400">
              {formatBytes(remainingBytes)} remaining
            </p>
          </>
        )}
      </div>

      {files.length === 0 ? (
        <p className="text-gray-400">No uploads yet</p>
      ) : (
        <div>
          <h3 className="font-semibold mb-3">Your Uploads</h3>
          <div className="space-y-2 max-h-64 overflow-y-auto">
            {files.map((file) => (
              <div
                key={file.id}
                className="flex items-center justify-between bg-gray-700 p-3 rounded"
              >
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-mono truncate">{file.filename}</p>
                  <p className="text-xs text-gray-400">
                    {formatBytes(file.size_bytes)} • {formatDate(file.created_at)}
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => deleteUpload.mutate(file.id)}
                  disabled={deleteUpload.isPending}
                  className="ml-2"
                >
                  Delete
                </Button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
