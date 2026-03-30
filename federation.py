"""Federation protocol routes for BSCP"""
from flask import Blueprint, request, jsonify, current_app
import requests
import traceback
from json_discovery import get_endpoint

federation_bp = Blueprint('federation', __name__)


@federation_bp.route("/federation/receive", methods=["POST"])
def receive_message():
    try:
        from app import Message
        db = current_app.extensions['sqlalchemy']
        data = request.json
        print(f"[FEDERATION] Received message: {data}")

        if '@' not in data['sender']:
            print(f"[FEDERATION] Invalid sender format: {data['sender']}")
            return "Invalid sender format", 400

        sender_domain = data['sender'].split('@')[-1]
        val_params = {"messageId": data['id'], "validationKey": data['validationKey'], "sender": data['sender'], "receiver": data['receiver']}
        val_url = get_endpoint(sender_domain, "userserver", "federation_validate")

        if not val_url:
            val_url = f"http://{sender_domain}/federation/validate"  # Fallback
        print(f"[FEDERATION] Validating at: {val_url}")

        try:
            val_resp = requests.get(val_url, params=val_params, timeout=3)
            print(f"[FEDERATION] Validation response: {val_resp.status_code} - {val_resp.text}")
            if val_resp.json().get("valid"):
                # Keep receiver with domain (sender already normalized it)
                received = Message(id=data['id'], sender=data['sender'], receiver=data['receiver'], text=data['text'], validation_key=data.get('validationKey'))
                print(f"[FEDERATION] Saving message: sender={received.sender}, receiver={received.receiver}")
                db.session.add(received)
                db.session.commit()
                print(f"[FEDERATION] Message saved successfully")
                return "OK", 200
            else:
                print(f"[FEDERATION] Validation failed")
        except Exception as e:
            print(f"[FEDERATION] Validation error: {e}")
            import traceback
            traceback.print_exc()
    except Exception as e:
        print(f"[FEDERATION] Error: {e}")
        import traceback
        traceback.print_exc()
    return "Invalid", 401


@federation_bp.route("/federation/validate")
def validate_message():
    try:
        from app import Message
        db = current_app.extensions['sqlalchemy']
        msg = db.session.query(Message).filter_by(id=request.args.get("messageId")).first()
        if msg and msg.validation_key == request.args.get("validationKey") and msg.sender == request.args.get("sender") and msg.receiver == request.args.get("receiver"):
            return jsonify({"valid": True})
        return jsonify({"valid": False})
    except Exception as e:
        print(f"Validate message error: {e}")
        traceback.print_exc()
        return jsonify({"valid": False, "error": str(e)}), 500
