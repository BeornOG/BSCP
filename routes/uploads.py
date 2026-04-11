"""File upload routes — /api/upload"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from werkzeug.utils import secure_filename
import uuid, os

from schemas import UploadResponse
from routes import require_auth


uploads_blp = SmorestBlueprint("uploads", __name__, url_prefix="/api/upload",
                                description="File uploads")


@uploads_blp.route("/")
class UploadResource(MethodView):
    @uploads_blp.response(201, UploadResponse)
    def post(self):
        """Upload a file and get a markdown embed"""
        require_auth()
        from app import Upload, DOMAIN
        db = current_app.extensions['sqlalchemy']

        if "file" not in request.files:
            abort(400, message="No file provided")
        file = request.files["file"]
        if file.filename == "":
            abort(400, message="No filename")

        mimetype = file.mimetype or "application/octet-stream"
        filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
        file.save(os.path.join(current_app.config["UPLOAD_FOLDER"], filename))

        upload = Upload(
            filename=filename,
            mimetype=mimetype,
            uploaded_by=request.user.id,
        )
        db.session.add(upload)
        db.session.commit()

        file_url = f"http://{DOMAIN}/uploads/{filename}"
        return {"url": file_url, "mimetype": mimetype, "markdown": f"![image]({file_url})"}
