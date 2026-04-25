import Avatar from './Avatar';
import Button from './Button';
import type { UserProfile } from '../../types';

interface ProfileModalProps {
  isOpen: boolean;
  onClose: () => void;
  profile?: UserProfile;
  isLoading?: boolean;
}

export function ProfileModal({ isOpen, onClose, profile, isLoading }: ProfileModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div
        className="bg-[#141517] rounded-lg border border-[#232529] p-8 max-w-sm w-full mx-4 space-y-6"
        onClick={(e) => e.stopPropagation()}
      >
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <div className="animate-spin w-8 h-8 border-2 border-[#71747a] border-t-[var(--accent)] rounded-full" />
          </div>
        ) : profile ? (
          <>
            {/* Header with Avatar and Name */}
            <div className="flex flex-col items-center space-y-3">
              <Avatar src={profile.profile_pic} name={profile.display_name} size="xl" status={profile.status as any} />
              <div className="text-center">
                <h2 className="text-lg font-semibold text-[#e8eaed]">{profile.display_name}</h2>
                <p className="text-sm text-[#71747a]">{profile.username}</p>
              </div>
            </div>

            {/* Status */}
            <div className="space-y-2">
              <p className="text-xs font-medium text-[#71747a] uppercase">Status</p>
              <div className="flex items-center gap-2">
                <div
                  className={`w-2 h-2 rounded-full ${
                    profile.status === 'online'
                      ? 'bg-green-500'
                      : profile.status === 'away'
                        ? 'bg-yellow-500'
                        : profile.status === 'dnd'
                          ? 'bg-red-500'
                          : 'bg-[#71747a]'
                  }`}
                />
                <span className="text-sm text-[#e8eaed] capitalize">{profile.status || 'offline'}</span>
              </div>
            </div>

            {/* Bio - if available */}
            {profile.bio && (
              <div className="space-y-2">
                <p className="text-xs font-medium text-[#71747a] uppercase">Bio</p>
                <p className="text-sm text-[#e8eaed] whitespace-pre-wrap break-words">{profile.bio}</p>
              </div>
            )}

            {/* Role */}
            {profile.is_admin && (
              <div className="p-2 rounded bg-[var(--accent)]/10 border border-[var(--accent)]/30">
                <p className="text-xs text-[var(--accent)] font-medium">Admin</p>
              </div>
            )}

            {/* Actions */}
            <div className="flex gap-2 pt-2">
              <Button
                onClick={onClose}
                className="flex-1 bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
              >
                Close
              </Button>
            </div>
          </>
        ) : (
          <p className="text-sm text-red-400">Failed to load profile</p>
        )}
      </div>
    </div>
  );
}
