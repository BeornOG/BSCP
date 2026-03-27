"""Federation protocol routes for BSCP"""
from flask import Blueprint, request, jsonify, current_app
import requests
import traceback
from kdl_discovery import get_endpoint

federation_bp = Blueprint('federation', __name__)


@federation_bp.route("/federation/receive", methods=["POST"])
def receive_message():
    try:
        from app import Message
        db = current_app.extensions['sqlalchemy']
        data = request.json
        sender_domain = data['sender'].split('@')[-1]
        val_params = {"messageId": data['id'], "validationKey": data['validationKey']}

        val_url = get_endpoint(sender_domain, "userserver", "federation_validate")
        if not val_url:
            val_url = f"http://{sender_domain}/federation/validate"  # Fallback

        try:
            val_resp = requests.get(val_url, params=val_params, timeout=3)
            if val_resp.json().get("valid"):
                received = Message(id=data['id'], sender=data['sender'], receiver=data['receiver'], text=data['text'], validation_key=data['validationKey'])
                db.session.add(received)
                db.session.commit()
                return "OK", 200
        except Exception as e:
            print(f"Validation error: {e}")
            traceback.print_exc()
        return "Invalid", 401
    except Exception as e:
        print(f"Receive message error: {e}")
        traceback.print_exc()
        return "Error", 500


@federation_bp.route("/federation/validate")
def validate_message():
    try:
        from app import Message
        db = current_app.extensions['sqlalchemy']
        msg = db.session.query(Message).filter_by(id=request.args.get("messageId")).first()
        if msg and msg.validation_key == request.args.get("validationKey"):
            return jsonify({"valid": True})
        return jsonify({"valid": False})
    except Exception as e:
        print(f"Validate message error: {e}")
        traceback.print_exc()
        return jsonify({"valid": False, "error": str(e)}), 500
