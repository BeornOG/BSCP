import { useState, useEffect, useRef } from 'react';
import { useProfile, useUpdateProfile, useUploadProfilePic, useDeleteProfilePic } from '../hooks/useProfile';
import { useWebhooks, useCreateWebhook, useDeleteWebhook, useRegenerateWebhook } from '../hooks/useWebhooks';
import { useTwoFactorSetup, useTwoFactorEnable, useTwoFactorDisable } from '../hooks/use2FA';
import { useLogout } from '../hooks/useAuth';
import { Avatar, Button, Input, Spinner } from '../components/ui';
import MediaManager from '../components/settings/MediaManager';
import StorageSettings from '../components/settings/StorageSettings';
import Connections from '../components/settings/Connections';

const ACCENT_COLORS = ['#6e8efb', '#ff716c', '#e9caf0', '#4d3755', '#28a745'];

export default function SettingsPage() {
  const { data: profile, isLoading } = useProfile();
  const { data: webhooks = [] } = useWebhooks();
  const updateProfile = useUpdateProfile();
  const uploadPic = useUploadProfilePic();
  const deletePic = useDeleteProfilePic();
  const logout = useLogout();
  const createWebhook = useCreateWebhook();
  const deleteWebhook = useDeleteWebhook();
  const regenerateWebhook = useRegenerateWebhook();
  const twoFactorSetup = useTwoFactorSetup();
  const twoFactorEnable = useTwoFactorEnable();
  const twoFactorDisable = useTwoFactorDisable();

  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'dark');
  const [accentColor, setAccentColor] = useState(() => localStorage.getItem('accent_color') || '#6e8efb');
  const [enableChime, setEnableChime] = useState(() => localStorage.getItem('notif_chime') !== 'false');
  const [enableDesktopNotif, setEnableDesktopNotif] = useState(() => localStorage.getItem('notif_desktop') !== 'false');
  const [showWebhookDialog, setShowWebhookDialog] = useState(false);
  const [webhookName, setWebhookName] = useState('');
  const [webhookAvatarUrl, setWebhookAvatarUrl] = useState('');
  const [copiedWebhookId, setCopiedWebhookId] = useState<string | null>(null);
  const [show2FADialog, setShow2FADialog] = useState(false);
  const [show2FAVerify, setShow2FAVerify] = useState(false);
  const [show2FADisable, setShow2FADisable] = useState(false);
  const [twoFAQRCode, setTwoFAQRCode] = useState('');
  const [twoFASecret, setTwoFASecret] = useState('');
  const [twoFACode, setTwoFACode] = useState('');
  const [disablePassword, setDisablePassword] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (profile) {
      setDisplayName(profile.display_name);
      setBio(profile.bio || '');
    }
  }, [profile]);

  useEffect(() => {
    document.body.classList.remove('theme-dark', 'theme-light');
    document.body.classList.add(`theme-${theme}`);
    localStorage.setItem('theme', theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.style.setProperty('--accent', accentColor);
    localStorage.setItem('accent_color', accentColor);
  }, [accentColor]);

  useEffect(() => {
    localStorage.setItem('notif_chime', enableChime ? 'true' : 'false');
  }, [enableChime]);

  useEffect(() => {
    localStorage.setItem('notif_desktop', enableDesktopNotif ? 'true' : 'false');
  }, [enableDesktopNotif]);

  const handleSave = () => {
    updateProfile.mutate({ display_name: displayName, bio: bio || undefined });
  };

  const handlePicUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) uploadPic.mutate(file);
  };

  const handleCreateWebhook = () => {
    if (!webhookName.trim()) return;
    createWebhook.mutate({
      name: webhookName,
      avatar_url: webhookAvatarUrl || undefined,
    }, {
      onSuccess: () => {
        setWebhookName('');
        setWebhookAvatarUrl('');
        setShowWebhookDialog(false);
      },
    });
  };

  const handleCopyWebhook = (url: string, webhookId: string) => {
    navigator.clipboard.writeText(url);
    setCopiedWebhookId(webhookId);
    setTimeout(() => setCopiedWebhookId(null), 2000);
  };

  const handleStartTwoFASetup = () => {
    twoFactorSetup.mutate(undefined, {
      onSuccess: (data) => {
        setTwoFAQRCode(data.qr_code);
        setTwoFASecret(data.secret);
        setShow2FADialog(true);
        setShow2FAVerify(false);
        setTwoFACode('');
      },
    });
  };

  const handleVerifyTwoFA = async () => {
    if (!twoFACode.trim() || twoFACode.length !== 6) return;
    twoFactorEnable.mutate(twoFACode, {
      onSuccess: async (response: any) => {
        if (response.success) {
          // Wait for profile to refetch before closing dialog
          await new Promise(resolve => setTimeout(resolve, 500));
          setShow2FADialog(false);
          setShow2FAVerify(false);
          setTwoFACode('');
          setTwoFASecret('');
          setTwoFAQRCode('');
        }
      },
    });
  };

  const handleDisableTwoFA = () => {
    if (!disablePassword.trim()) return;
    twoFactorDisable.mutate(disablePassword, {
      onSuccess: () => {
        setShow2FADisable(false);
        setDisablePassword('');
      },
    });
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-10 space-y-10">
      <h1 className="text-2xl font-semibold text-[#e8eaed]">Settings</h1>

      {/* Profile Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Profile</h2>
        <div className="flex items-center gap-6">
          <div className="relative group cursor-pointer" onClick={() => fileInputRef.current?.click()}>
            <Avatar name={profile?.display_name || ''} src={profile?.profile_pic} size="xl" />
            <div className="absolute inset-0 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
              <span className="text-xs text-white">Change</span>
            </div>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={handlePicUpload}
            />
          </div>
          <div className="flex-1 space-y-2">
            <Input
              placeholder="Display name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
            <textarea
              placeholder="Bio"
              value={bio}
              onChange={(e) => setBio(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 rounded border border-[#232529] bg-[#0a0a0b] text-sm text-[#e8eaed] placeholder-[#71747a] outline-none focus:border-[var(--accent)]"
            />
            {profile?.profile_pic && (
              <button
                onClick={() => deletePic.mutate()}
                className="text-xs text-red-400 hover:text-red-300 transition-colors"
              >
                Remove photo
              </button>
            )}
          </div>
        </div>
      </section>

      {/* Theme Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Theme</h2>
        <div className="flex gap-3">
          <button
            onClick={() => setTheme('dark')}
            className={`flex-1 px-4 py-3 rounded-lg border text-sm transition-colors ${
              theme === 'dark'
                ? 'border-[var(--accent)] bg-[var(--accent)]/10 text-[#e8eaed]'
                : 'border-[#232529] bg-[#141517] text-[#71747a] hover:border-[#71747a]'
            }`}
          >
            Midnight Slate
          </button>
          <button
            onClick={() => setTheme('light')}
            className={`flex-1 px-4 py-3 rounded-lg border text-sm transition-colors ${
              theme === 'light'
                ? 'border-[var(--accent)] bg-[var(--accent)]/10 text-[#e8eaed]'
                : 'border-[#232529] bg-[#141517] text-[#71747a] hover:border-[#71747a]'
            }`}
          >
            Alabaster Muse
          </button>
        </div>
      </section>

      {/* Accent Color Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Accent Color</h2>
        <div className="flex gap-3">
          {ACCENT_COLORS.map((color) => (
            <button
              key={color}
              onClick={() => setAccentColor(color)}
              className={`w-10 h-10 rounded-full transition-transform ${
                accentColor === color ? 'ring-2 ring-offset-2 ring-offset-[#0a0a0b] ring-[#e8eaed] scale-110' : ''
              }`}
              style={{ backgroundColor: color }}
              aria-label={`Select color ${color}`}
            />
          ))}
        </div>
      </section>

      {/* Notifications Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Notifications</h2>
        <div className="space-y-3">
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={enableDesktopNotif}
              onChange={(e) => setEnableDesktopNotif(e.target.checked)}
              className="w-5 h-5 rounded border-[#232529] bg-[#141517] accent-[var(--accent)]"
            />
            <span className="text-sm text-[#e8eaed]">Desktop notifications</span>
          </label>
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={enableChime}
              onChange={(e) => setEnableChime(e.target.checked)}
              className="w-5 h-5 rounded border-[#232529] bg-[#141517] accent-[var(--accent)]"
            />
            <span className="text-sm text-[#e8eaed]">Message chime sound</span>
          </label>
        </div>
      </section>

      {/* Security Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Security</h2>
        <div className="p-4 rounded-lg bg-[#141517] border border-[#232529] space-y-4">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <h3 className="text-sm font-medium text-[#e8eaed]">Two-Factor Authentication</h3>
              <p className="text-xs text-[#71747a]">
                {profile?.is_2fa_enabled ? 'Enabled' : 'Add an extra layer of security to your account'}
              </p>
            </div>
            {profile?.is_2fa_enabled ? (
              <Button
                onClick={() => setShow2FADisable(true)}
                className="text-xs px-3 py-1.5 bg-red-600/20 text-red-400 hover:bg-red-600/30 border border-red-600/50"
              >
                Disable
              </Button>
            ) : (
              <Button
                onClick={handleStartTwoFASetup}
                disabled={twoFactorSetup.isPending}
                className="text-xs px-3 py-1.5 bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
              >
                {twoFactorSetup.isPending ? 'Loading...' : 'Enable'}
              </Button>
            )}
          </div>
        </div>

        {show2FADisable && (
          <div className="p-4 rounded-lg bg-red-600/10 border border-red-600/30 space-y-3">
            <p className="text-sm text-red-300">Enter your password to disable 2FA</p>
            <Input
              type="password"
              placeholder="Password"
              value={disablePassword}
              onChange={(e) => setDisablePassword(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleDisableTwoFA()}
            />
            <div className="flex gap-2 justify-end">
              <Button
                onClick={() => {
                  setShow2FADisable(false);
                  setDisablePassword('');
                }}
                className="text-xs bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
              >
                Cancel
              </Button>
              <Button
                onClick={handleDisableTwoFA}
                disabled={twoFactorDisable.isPending || !disablePassword.trim()}
                className="text-xs bg-red-600 text-white hover:bg-red-700"
              >
                {twoFactorDisable.isPending ? 'Disabling...' : 'Disable 2FA'}
              </Button>
            </div>
          </div>
        )}
      </section>

      <Connections />

      {/* Webhooks Section */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Webhooks</h2>
          <Button
            onClick={() => setShowWebhookDialog(true)}
            className="text-xs px-3 py-1.5 bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
          >
            Create
          </Button>
        </div>

        {webhooks.length === 0 ? (
          <p className="text-sm text-[#71747a]">No webhooks yet. Create one to receive messages via incoming webhooks.</p>
        ) : (
          <div className="space-y-3">
            {webhooks.map((webhook) => (
              <div key={webhook.id} className="p-4 rounded-lg bg-[#141517] border border-[#232529] space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    {webhook.profile_pic && (
                      <img src={webhook.profile_pic} alt={webhook.name} className="w-8 h-8 rounded-full" />
                    )}
                    <span className="text-sm font-medium text-[#e8eaed]">{webhook.name}</span>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleCopyWebhook(webhook.url, webhook.id)}
                      className="text-xs px-2 py-1 rounded bg-[#232529] text-[#71747a] hover:bg-[#333] transition-colors"
                    >
                      {copiedWebhookId === webhook.id ? 'Copied!' : 'Copy URL'}
                    </button>
                    <button
                      onClick={() => regenerateWebhook.mutate(webhook.id)}
                      className="text-xs px-2 py-1 rounded bg-[#232529] text-[#71747a] hover:bg-[#333] transition-colors"
                      disabled={regenerateWebhook.isPending}
                    >
                      {regenerateWebhook.isPending ? 'Regen...' : 'Regenerate'}
                    </button>
                    <button
                      onClick={() => deleteWebhook.mutate(webhook.id)}
                      className="text-xs px-2 py-1 rounded bg-[#232529] text-red-400 hover:bg-red-400/20 transition-colors"
                      disabled={deleteWebhook.isPending}
                    >
                      {deleteWebhook.isPending ? 'Deleting...' : 'Delete'}
                    </button>
                  </div>
                </div>
                <div className="text-xs text-[#71747a] space-y-1">
                  <div>Created: {new Date(webhook.created_at * 1000).toLocaleDateString()}</div>
                  {webhook.last_used && <div>Last used: {new Date(webhook.last_used * 1000).toLocaleString()}</div>}
                </div>
              </div>
            ))}
          </div>
        )}

        {showWebhookDialog && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-[#141517] rounded-lg border border-[#232529] p-6 max-w-sm w-full mx-4 space-y-4">
              <h3 className="text-lg font-semibold text-[#e8eaed]">Create Webhook</h3>
              <Input
                placeholder="Webhook name (e.g., GitHub)"
                value={webhookName}
                onChange={(e) => setWebhookName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateWebhook()}
              />
              <Input
                placeholder="Avatar URL (optional)"
                value={webhookAvatarUrl}
                onChange={(e) => setWebhookAvatarUrl(e.target.value)}
              />
              <div className="flex gap-3 justify-end">
                <Button
                  onClick={() => {
                    setShowWebhookDialog(false);
                    setWebhookName('');
                    setWebhookAvatarUrl('');
                  }}
                  className="bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
                >
                  Cancel
                </Button>
                <Button
                  onClick={handleCreateWebhook}
                  disabled={createWebhook.isPending || !webhookName.trim()}
                  className="bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
                >
                  {createWebhook.isPending ? 'Creating...' : 'Create'}
                </Button>
              </div>
            </div>
          </div>
        )}
      </section>

      {/* Media Manager Section */}
      <section className="space-y-4">
        <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Media Storage</h2>
        <div className="p-4 rounded-lg bg-[#141517] border border-[#232529]">
          <MediaManager />
        </div>
      </section>

      {/* Admin Storage Settings */}
      {profile?.is_admin && (
        <section className="space-y-4">
          <div className="p-4 rounded-lg bg-[#1a1d21] border border-[#2d2e31]">
            <StorageSettings />
          </div>
        </section>
      )}

      {/* Actions */}
      <div className="flex items-center gap-4 pt-4 border-t border-[#232529]">
        <Button onClick={handleSave} disabled={updateProfile.isPending}>
          {updateProfile.isPending ? 'Saving...' : 'Save changes'}
        </Button>
        <Button
          onClick={() => logout.mutate()}
          className="bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
        >
          Log out
        </Button>
      </div>

      {/* 2FA Setup Modal */}
      {show2FADialog && !show2FAVerify && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#141517] rounded-lg border border-[#232529] p-6 max-w-sm w-full mx-4 space-y-4">
            <h3 className="text-lg font-semibold text-[#e8eaed]">Set up Two-Factor Authentication</h3>
            <p className="text-sm text-[#71747a]">Scan this QR code with your authenticator app:</p>
            {twoFAQRCode && (
              <div className="flex justify-center bg-white p-4 rounded">
                <img src={`data:image/png;base64,${twoFAQRCode}`} alt="2FA QR Code" className="w-48 h-48" />
              </div>
            )}
            <div className="space-y-2">
              <p className="text-xs text-[#71747a]">Or enter this code manually:</p>
              <div className="p-3 rounded bg-[#0a0a0b] border border-[#232529] font-mono text-sm text-[#e8eaed] break-all">
                {twoFASecret}
              </div>
            </div>
            <div className="flex gap-3 justify-end pt-2">
              <Button
                onClick={() => {
                  setShow2FADialog(false);
                  setTwoFAQRCode('');
                  setTwoFASecret('');
                }}
                className="bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
              >
                Cancel
              </Button>
              <Button
                onClick={() => setShow2FAVerify(true)}
                className="bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
              >
                Next
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* 2FA Verify Modal */}
      {show2FADialog && show2FAVerify && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#141517] rounded-lg border border-[#232529] p-6 max-w-sm w-full mx-4 space-y-4">
            <h3 className="text-lg font-semibold text-[#e8eaed]">Verify Your Code</h3>
            <p className="text-sm text-[#71747a]">Enter the 6-digit code from your authenticator app:</p>
            <Input
              maxLength={6}
              placeholder="000000"
              value={twoFACode}
              onChange={(e) => setTwoFACode(e.target.value.replace(/[^0-9]/g, ''))}
              onKeyDown={(e) => e.key === 'Enter' && twoFACode.length === 6 && handleVerifyTwoFA()}
              className="text-center text-xl tracking-widest"
            />
            {twoFactorEnable.isError && (
              <p className="text-sm text-red-400">Invalid code. Please try again.</p>
            )}
            <div className="flex gap-3 justify-end">
              <Button
                onClick={() => {
                  setShow2FADialog(false);
                  setShow2FAVerify(false);
                  setTwoFACode('');
                }}
                className="bg-transparent border border-[#232529] text-[#e8eaed] hover:bg-[#141517]"
              >
                Cancel
              </Button>
              <Button
                onClick={handleVerifyTwoFA}
                disabled={twoFactorEnable.isPending || twoFACode.length !== 6}
                className="bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
              >
                {twoFactorEnable.isPending ? 'Verifying...' : 'Verify'}
              </Button>
            </div>
          </div>
        </div>
      )}
      </div>
    </div>
  );
}
