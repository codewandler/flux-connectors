op intercom-contact-note-create(contact_id: String, body: String, admin_id: String) -> Any
  description "Add an internal note to a contact, visible to teammates in the workspace and never to the contact. Adding the same note twice adds two notes. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{host}"
  url = fmt("{base}/contacts/{contact_id}/notes")
  content_type = "application/json"
  payload = { admin_id, body }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
