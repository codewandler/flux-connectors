op box-folder-get(folder_id: String) -> Any
  description "Get one folder's metadata: name, size, timestamps and parent. Does not return its contents — use box-folder-items-list for that. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/folders/{folder_id}")
  response = http.request(method: "GET", url)
  return response
