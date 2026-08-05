op dropbox-folder-create(path: String) -> Any
  description "Create a new folder at the given path, including any missing parent folders. Naming a path that already exists answers 409 conflict rather than creating a duplicate, since autorename is not offered by this connector. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/files/create_folder_v2")
  content_type = "application/json"
  payload = { path }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
