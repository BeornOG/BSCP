export interface Message {
  id: string;
  sender: string;
  receiver: string;
  text: string;
  timestamp: number;
  is_read: boolean;
}

export interface Chat {
  id: string;
  display_name: string;
  profile_pic: string | null;
}

export interface UserProfile {
  id: string;
  username: string;
  domain: string;
  full_id: string;
  display_name: string;
  profile_pic: string | null;
  is_admin: boolean;
  is_2fa_enabled: boolean;
  settings?: {
    theme: string;
    accent_color: string;
  };
}

export interface Invite {
  id: number;
  code: string;
  status: string;
  created_at: number | null;
  expires_at: number | null;
  used_by: string | null;
}

export interface AdminUser {
  id: string;
  username: string;
  domain: string;
  full_id: string;
  display_name: string;
  profile_pic: string | null;
  is_admin: boolean;
  is_2fa_enabled: boolean;
}
