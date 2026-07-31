op microsoft_graph-files-item-get(item_id: String) -> Any
  description "Get one OneDrive item's metadata by id: its name, size and where it lives. This never returns file content — downloading needs the separate /content endpoint, which returns a binary body and is not shipped by this connector. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/drive/items/{item_id}")
  response = http.request(method: "GET", url)
  return response
