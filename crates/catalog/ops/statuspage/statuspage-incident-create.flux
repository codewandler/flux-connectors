op statuspage-incident-create(name: String, status: String, body: String, deliver_notifications: Bool) -> Any
  description "Open a new incident on this status page. It is public the moment this call returns — anyone loading the page sees it, and with deliver_notifications true Statuspage emails and texts every subscriber the page has. The incident itself is reversible (resolve it, or delete it), but a notification that has been sent cannot be recalled, so treat deliver_notifications as the irreversible half of this call. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.statuspage.io/v1/pages/{page_id}"
  url = fmt("{base}/incidents")
  content_type = "application/json"
  payload = { incident: { body, deliver_notifications, name, status } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
