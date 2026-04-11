"""Marshmallow schemas for BSCP API - standardized request/response objects."""
from marshmallow import Schema, fields


# ---------------------------------------------------------------------------
# Shared / Reusable Schemas
# ---------------------------------------------------------------------------

class UserObject(Schema):
    """Standard representation of a user across the API."""
    id = fields.String(dump_only=True, metadata={"description": "Unique user ID (UUID)"})
    username = fields.String(required=True, metadata={"description": "Unique username"})
    domain = fields.String(dump_only=True, metadata={"description": "Server domain (e.g. localhost:5000)"})
    full_id = fields.String(dump_only=True, metadata={"description": "Federated identity (username@domain)"})
    display_name = fields.String(metadata={"description": "Display name shown in chat"})
    profile_pic = fields.String(load_default=None, metadata={"description": "URL to profile picture"})
    is_admin = fields.Boolean(dump_only=True, metadata={"description": "Whether user is an admin"})
    is_2fa_enabled = fields.Boolean(dump_only=True, metadata={"description": "Whether 2FA is enabled"})


class MessageObject(Schema):
    """Standard representation of a chat message."""
    id = fields.String(dump_only=True, metadata={"description": "Unique message ID (domain/uuid)"})
    sender = fields.String(required=True, metadata={"description": "Sender's federated identity (user@domain)"})
    receiver = fields.String(required=True, metadata={"description": "Receiver's federated identity (user@domain)"})
    text = fields.String(required=True, metadata={"description": "Message content (supports markdown)"})
    timestamp = fields.Float(dump_only=True, metadata={"description": "Unix epoch timestamp"})
    is_read = fields.Boolean(dump_only=True, metadata={"description": "Whether message has been read"})


class ChatObject(Schema):
    """Standard representation of a chat conversation."""
    id = fields.String(dump_only=True, metadata={"description": "Chat partner's federated identity"})
    display_name = fields.String(dump_only=True, metadata={"description": "Display name of chat partner"})
    profile_pic = fields.String(dump_default=None, metadata={"description": "Profile picture URL"})
    status = fields.String(dump_only=True, metadata={"description": "User status: online, offline, away, or dnd"})


class InviteObject(Schema):
    """Standard representation of an invite code."""
    id = fields.Integer(dump_only=True)
    code = fields.String(dump_only=True, metadata={"description": "Invite code string"})
    status = fields.String(dump_only=True, metadata={"description": "Current status: available or used"})
    created_at = fields.Float(dump_only=True, metadata={"description": "Creation timestamp (unix epoch)"})
    expires_at = fields.Float(dump_only=True, metadata={"description": "Expiry timestamp (unix epoch)"})
    used_by = fields.String(dump_default=None, metadata={"description": "User ID who used this code"})


# ---------------------------------------------------------------------------
# User Profile Schemas
# ---------------------------------------------------------------------------

class UserProfile(Schema):
    """Public user profile — never exposes internal IDs."""
    username = fields.String(metadata={"description": "Federated identity (username@domain)"})
    display_name = fields.String(metadata={"description": "Display name"})
    profile_pic = fields.String(allow_none=True, metadata={"description": "Profile picture URL"})
    status = fields.String(metadata={"description": "User status: online, offline, away, or dnd"})


class UserSettingsUpdate(Schema):
    """Request body for updating user settings."""
    display_name = fields.String(metadata={"description": "New display name"})


class ProfilePicResponse(Schema):
    """Response after uploading/deleting profile picture."""
    profile_pic = fields.String(allow_none=True, metadata={"description": "New profile picture URL"})


# ---------------------------------------------------------------------------
# Auth Schemas
# ---------------------------------------------------------------------------

class LoginRequest(Schema):
    """Login credentials."""
    user = fields.String(required=True, metadata={"description": "Username"})
    password = fields.String(required=True, metadata={"description": "Password"})


class LoginResponse(Schema):
    """Login result."""
    success = fields.Boolean(metadata={"description": "Whether login succeeded"})
    requires_2fa = fields.Boolean(load_default=False, metadata={"description": "Whether 2FA verification is needed"})
    error = fields.String(load_default=None, metadata={"description": "Error message if login failed"})


class TwoFactorRequest(Schema):
    """2FA verification code."""
    otp = fields.String(required=True, metadata={"description": "6-digit OTP code"})


class SetupRequest(Schema):
    """First-time admin account setup."""
    username = fields.String(required=True, metadata={"description": "Admin username"})
    email = fields.String(load_default=None, metadata={"description": "Admin email (optional)"})
    password = fields.String(required=True, metadata={"description": "Password"})
    password_confirm = fields.String(required=True, metadata={"description": "Password confirmation"})


class RegisterRequest(Schema):
    """User registration with invite code."""
    username = fields.String(required=True, metadata={"description": "Desired username"})
    password = fields.String(required=True, metadata={"description": "Password"})
    password_confirm = fields.String(required=True, metadata={"description": "Password confirmation"})
    invite_code = fields.String(required=True, metadata={"description": "Valid invite code"})


class AuthSuccessResponse(Schema):
    """Generic auth success response."""
    success = fields.Boolean(metadata={"description": "Whether the operation succeeded"})


class AuthErrorResponse(Schema):
    """Generic auth error response."""
    error = fields.String(metadata={"description": "Error message"})
    errors = fields.List(fields.String(), metadata={"description": "List of validation errors"})


class SetupStatusResponse(Schema):
    """Whether initial setup is needed."""
    needs_setup = fields.Boolean(metadata={"description": "True if no admin account exists yet"})


# ---------------------------------------------------------------------------
# Messaging Schemas
# ---------------------------------------------------------------------------

class SendMessageBody(Schema):
    """Request body for sending a message (receiver is in the URL)."""
    text = fields.String(required=True, metadata={"description": "Message content"})


class MessagesQueryArgs(Schema):
    """Query parameters for fetching messages."""
    since = fields.Float(load_default=None, metadata={"description": "Get messages after this unix timestamp"})
    before = fields.Float(load_default=None, metadata={"description": "Get messages before this unix timestamp"})
    limit = fields.Integer(load_default=50, metadata={"description": "Max messages to return (default 50)"})


# ---------------------------------------------------------------------------
# Upload Schemas
# ---------------------------------------------------------------------------

class UploadResponse(Schema):
    """Response after uploading a file."""
    url = fields.String(metadata={"description": "Direct URL to the uploaded file"})
    mimetype = fields.String(metadata={"description": "MIME type of the uploaded file"})
    markdown = fields.String(metadata={"description": "Markdown embed tag for the file"})


# ---------------------------------------------------------------------------
# Admin Schemas
# ---------------------------------------------------------------------------

class BatchProfileRequest(Schema):
    """Request to fetch profiles for multiple users."""
    senders = fields.List(fields.String(), required=True, metadata={"description": "List of federated user IDs"})


class BatchProfileResponse(Schema):
    """Map of user ID to profile picture URL."""
    # Dynamic keys, represented as dict
    pass
