op google-drive-file-update(file_id: String, name: String) -> Any
  description "Rename a Drive file: sets its name and changes nothing else. It does not move, share, trash or replace the file's content. Needs the `drive.file` scope for files this token created, or `drive` for any other. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://www.googleapis.com"
  url = fmt("{base}/drive/v3/files/{file_id}")
  content_type = "application/json"
  payload = { name }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
