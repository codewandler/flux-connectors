op dropbox-folder-list(path: String) -> Any
  description "List the files and folders directly inside a folder, first page only. Each entry names its own type (file, folder, or deleted), name and full path. Direction remains conservatively authored as write pending individual review; POST is transport only and supplied no evidence. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/files/list_folder")
  content_type = "application/json"
  payload = { path }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
