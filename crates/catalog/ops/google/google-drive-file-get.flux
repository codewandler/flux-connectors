op google-drive-file-get(file_id: String) -> Any
  description "Get one Drive file's metadata — its id, name and MIME type. This never returns file content: downloading needs `alt=media`, which is a query parameter and therefore unavailable (C-30). Needs the `drive.metadata.readonly` scope (or `drive.readonly`). A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://www.googleapis.com"
  url = fmt("{base}/drive/v3/files/{file_id}")
  response = http.request(method: "GET", url)
  return response
