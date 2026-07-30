op intercom-contact-get(contact_id: String) -> Any
  description "Get one Intercom contact by id — its role, email, name, custom attributes, tags and the companies it belongs to. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.intercom.io"
  url = fmt("{base}/contacts/{contact_id}")
  response = http.request(method: "GET", url)
  return response
