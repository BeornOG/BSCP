"""User lookup service — local and federated profile resolution."""
import requests as http_requests
from flask import current_app
from routes import get_user_status


def serialize_profile(user, domain):
    """Convert a User model to a public profile dict — no internal IDs exposed."""
    return {
        "username": f"{user.username}@{domain}",
        "display_name": user.display_name or user.username,
        "profile_pic": user.profile_pic or None,
        "status": get_user_status(user),
    }


def get_profile(full_id):
    """Resolve a user profile by username@domain.

    Local users are fetched from the database.
    Remote users are fetched via federation.

    Returns a profile dict, or None if the user was not found.
    Raises ConnectionError if a remote server is unreachable.
    """
    if "@" not in full_id:
        return None

    from app import User, DOMAIN
    from json_discovery import get_endpoint

    username, domain = full_id.rsplit("@", 1)
    db = current_app.extensions['sqlalchemy']

    if domain == DOMAIN:
        user = db.session.query(User).filter_by(
            username=username, is_deleted=False
        ).first()
        return serialize_profile(user, DOMAIN) if user else None

    # Remote — fetch via federation
    try:
        base = get_endpoint(domain, "userserver", "users")
        if not base:
            base = f"http://{domain}/api/users"
        resp = http_requests.get(f"{base}/{full_id}", timeout=3)
        if resp.status_code == 200:
            return resp.json()
        return None
    except http_requests.RequestException:
        raise ConnectionError(f"Failed to reach remote server {domain}")
