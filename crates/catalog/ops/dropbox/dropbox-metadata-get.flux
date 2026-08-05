op dropbox-metadata-get(path: String) -> Any
  description "Get metadata for a file or folder at a given path: its type, name, id, and — for a file — size and content hash. Does not return a folder's contents (use dropbox-folder-list) or a file's content (use dropbox-temporary-link-get). Direction remains conservatively authored as write pending individual review; POST is transport only and supplied no evidence. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/files/get_metadata")
  content_type = "application/json"
  payload = { path }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
