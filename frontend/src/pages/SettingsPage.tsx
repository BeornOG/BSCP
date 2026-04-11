import { useState, useEffect, useRef } from 'react';
import { useProfile, useUpdateProfile, useUploadProfilePic, useDeleteProfilePic } from '../hooks/useProfile';
import { useLogout } from '../hooks/useAuth';
import { Avatar, Button, Input, Spinner } from '../components/ui';

const ACCENT_COLORS = ['#6e8efb', '#ff716c', '#e9caf0', '#4d3755', '#28a745'];

export default function SettingsPage() {
  const { data: profile, isLoading } = useProfile();
  const updateProfile = useUpdateProfile();
  const uploadPic = useUploadProfilePic();
  const deletePic = useDeleteProfilePic();
  const logout = useLogout();

  const [displayName, setDisplayName] = useState('');
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'dark');
  const [accentColor, setAccentColor] = useState(() => localStorage.getItem('accent_color') || '#6e8efb');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (profile) {
      setDisplayName(profile.display_name);
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

  const handleSave = () => {
    updateProfile.mutate({ display_name: displayName });
  };

  const handlePicUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) uploadPic.mutate(file);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner />
      </div>
    );
  }

  return (
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
    </div>
  );
}
