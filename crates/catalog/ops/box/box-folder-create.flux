op box-folder-create(name: String, parent_id: String) -> Any
  description "Create a new, empty folder as a child of an existing folder. Naming a folder that already exists under the same parent answers 409 conflict rather than creating a duplicate. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/folders")
  content_type = "application/json"
  payload = { name, parent: { id: parent_id } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
