"""File upload routes — /api/upload"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from werkzeug.utils import secure_filename
import uuid, os

from schemas import UploadResponse, UserUploadsResponse, UploadObject
from routes import require_auth


uploads_blp = SmorestBlueprint("uploads", __name__, url_prefix="/api/upload",
                                description="File uploads")


@uploads_blp.route("/")
class UploadResource(MethodView):
    @uploads_blp.response(201, UploadResponse)
    def post(self):
        """Upload a file and get a markdown embed"""
        require_auth()
        from app import Upload, ServerConfig, DOMAIN
        db = current_app.extensions['sqlalchemy']

        if "file" not in request.files:
            abort(400, message="No file provided")
        file = request.files["file"]
        if file.filename == "":
            abort(400, message="No filename")

        # Skip storage limits for primary admin
        if not request.user.is_primary_admin:
            # Get user's individual storage limit
            limit_bytes = request.user.storage_limit_mb * 1024 * 1024

            # Read file into memory to get size
            file.seek(0, os.SEEK_END)
            file_size = file.tell()
            file.seek(0)

            if file_size > limit_bytes:
                abort(413, message=f"File exceeds size limit of {request.user.storage_limit_mb}MB")

            # Check user's total storage
            user_uploads = db.session.query(Upload).filter_by(uploaded_by=request.user.id).all()
            total_size = sum(u.size_bytes for u in user_uploads)

            if total_size + file_size > limit_bytes:
                abort(413, message=f"Insufficient storage. Used: {total_size // (1024*1024)}MB / {request.user.storage_limit_mb}MB")
        else:
            # Primary admin - just get file size
            file.seek(0, os.SEEK_END)
            file_size = file.tell()
            file.seek(0)

        mimetype = file.mimetype or "application/octet-stream"
        filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
        file.save(os.path.join(current_app.config["UPLOAD_FOLDER"], filename))

        upload = Upload(
            filename=filename,
            mimetype=mimetype,
            size_bytes=file_size,
            uploaded_by=request.user.id,
        )
        db.session.add(upload)
        db.session.commit()

        file_url = f"http://{DOMAIN}/uploads/{filename}"

        # Use appropriate markdown based on file type
        video_mimetypes = {'video/mp4', 'video/webm', 'video/ogg', 'video/quicktime', 'video/x-msvideo', 'video/x-flv', 'video/x-matroska'}
        if mimetype in video_mimetypes:
            # For videos, just return URL so frontend can convert to video tag
            markdown_url = file_url
        else:
            # For images and other files, use image markdown syntax
            markdown_url = f"![image]({file_url})"

        return {"url": file_url, "mimetype": mimetype, "markdown": markdown_url}


@uploads_blp.route("/<upload_id>", methods=["DELETE"])
@uploads_blp.response(204)
def delete_upload(upload_id):
    """Delete user's upload"""
    require_auth()
    from app import Upload
    db = current_app.extensions['sqlalchemy']

    upload = db.session.query(Upload).get(upload_id)
    if not upload:
        abort(404, message="Upload not found")

    if upload.uploaded_by != request.user.id:
        abort(403, message="Cannot delete other user's uploads")

    # Delete file from disk
    file_path = os.path.join(current_app.config["UPLOAD_FOLDER"], upload.filename)
    if os.path.exists(file_path):
        os.remove(file_path)

    db.session.delete(upload)
    db.session.commit()
    return None


@uploads_blp.route("/user/list")
class UserUploadsResource(MethodView):
    @uploads_blp.response(200, UserUploadsResponse)
    def get(self):
        """Get user's uploads and storage usage"""
        require_auth()
        from app import Upload
        db = current_app.extensions['sqlalchemy']

        uploads = db.session.query(Upload).filter_by(uploaded_by=request.user.id).all()
        total_size = sum(u.size_bytes for u in uploads)
        limit_bytes = request.user.storage_limit_mb * 1024 * 1024

        upload_objs = []
        for u in uploads:
            upload_objs.append({
                "id": u.id,
                "filename": u.filename,
                "mimetype": u.mimetype,
                "size_bytes": u.size_bytes,
                "created_at": u.created_at.timestamp(),
            })

        return {
            "uploads": upload_objs,
            "total_size_bytes": total_size,
            "limit_bytes": limit_bytes,
        }
