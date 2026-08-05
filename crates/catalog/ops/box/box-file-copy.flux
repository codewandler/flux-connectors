op box-file-copy(file_id: String, parent_id: String) -> Any
  description "Copy an existing file into a different folder, keeping its original name. Each call creates a new, independent file — copying the same file twice produces two copies, not one. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/files/{file_id}/copy")
  content_type = "application/json"
  payload = { parent: { id: parent_id } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
