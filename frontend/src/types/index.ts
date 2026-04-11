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
  status: 'online' | 'offline' | 'away' | 'dnd';
  unread_count: number;
}

export interface UserProfile {
  username: string;
  display_name: string;
  profile_pic: string | null;
  status: string;
  is_admin: boolean;
}

export interface Invite {
  id: number;
  code: string;
  status: string;
  created_at: number | null;
  expires_at: number | null;
  used_by: string | null;
}
