op microsoft_graph-files-item-update(item_id: String, name: String) -> Any
  description "Rename a OneDrive item: sets its name and changes nothing else. Does not move it — moving is a PATCH to the separate parentReference field, which this operation deliberately does not declare (C-56). A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/drive/items/{item_id}")
  content_type = "application/json"
  payload = { name }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
